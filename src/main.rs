use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use env_logger::Builder;
use log::LevelFilter;
use log::{error, info, warn};

use exitcode;

use futures::future::{loop_fn, Loop};
use futures::stream::Stream;
use futures::Future;

use tokio::net::TcpListener;
use tokio::prelude::FutureExt;
use tokio::runtime::Runtime;
use tokio::timer::Delay;

use tokio_signal;

use tk_listen::ListenExt;

use structopt;
use structopt::StructOpt;

#[cfg(all(unix, feature = "sandbox"))]
use rusty_sandbox::Sandbox;

#[cfg(all(unix, feature = "drop_privs"))]
use privdrop::PrivDrop;

#[cfg(all(unix, feature = "drop_privs"))]
use std::path::PathBuf;

#[cfg(all(unix, feature = "drop_privs"))]
use std::ffi::OsString;

static NUM_CLIENTS: AtomicUsize = AtomicUsize::new(0);
static BANNER: &[&str] = &[
    "My name is Yon",
    " Yonson\r\nI liv",
    "e in Wisconsin",
    ".\r\nThere, the ",
    "people I meet\r",
    "\nAs I walk dow",
    "n the street\r\n",
    "Say \"Hey, what",
    "'s your name?\"",
    "\r\nAnd I say:\r\n",
];

#[derive(Debug, StructOpt)]
#[structopt(name = "tarssh", about = "A SSH tarpit server")]
struct Config {
    /// Listen address(es) to bind to
    #[structopt(short = "l", long = "listen", default_value = "0.0.0.0:2222")]
    listen: Vec<SocketAddr>,
    /// Best-effort connection limit
    #[structopt(short = "c", long = "max-clients", default_value = "4096")]
    max_clients: u32,
    /// Seconds between responses
    #[structopt(short = "d", long = "delay", default_value = "10")]
    delay: u64,
    /// Socket write timeout
    #[structopt(short = "t", long = "timeout", default_value = "30")]
    timeout: u64,
    /// Verbose level (repeat for more verbosity)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
    /// Disable timestamps in logs
    #[structopt(long = "disable-timestamps")]
    disable_timestamps: bool,
    #[cfg(all(unix, feature = "drop_privs"))]
    #[structopt(flatten)]
    #[cfg(all(unix, feature = "drop_privs"))]
    privdrop: PrivDropConfig,
}

#[cfg(all(unix, feature = "drop_privs"))]
#[derive(Debug, StructOpt)]
struct PrivDropConfig {
    /// Run as this user and their primary group
    #[structopt(short = "u", long = "user", parse(from_os_str))]
    user: Option<OsString>,
    /// Run as this group
    #[structopt(short = "g", long = "group", parse(from_os_str))]
    group: Option<OsString>,
    /// Chroot to this directory
    #[structopt(long = "chroot", parse(from_os_str))]
    chroot: Option<PathBuf>,
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    Timeout,
    TimerFull,
    TimerShutdown,
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::error::Error;
        self.description().fmt(fmt)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(err) => err.description(),
            Error::Timeout => "timed out",
            Error::TimerFull => "timer at capacity",
            Error::TimerShutdown => "timer shutdown",
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<tokio::timer::Error> for Error {
    fn from(err: tokio::timer::Error) -> Self {
        if err.is_at_capacity() {
            Error::TimerFull
        } else {
            Error::TimerShutdown
        }
    }
}

impl From<tokio::timer::timeout::Error<std::io::Error>> for Error {
    fn from(err: tokio::timer::timeout::Error<std::io::Error>) -> Self {
        if err.is_elapsed() {
            Error::Timeout
        } else if err.is_inner() {
            Self::from(err.into_inner().expect("IO Error"))
        } else if err.is_timer() {
            Self::from(err.into_timer().expect("Timer error"))
        } else {
            panic!("unhandled timer error: {:?}", err);
        }
    }
}

fn errx<M: AsRef<str>>(code: i32, message: M) {
    error!("{}", message.as_ref());
    std::process::exit(code);
}

fn tarpit_connection(
    sock: tokio::net::TcpStream,
    peer: SocketAddr,
    delay: u64,
    timeout: u64,
) -> impl Future<Item = (), Error = ()> {
    let start = Instant::now();
    let _ = sock
        .set_recv_buffer_size(1)
        .map_err(|err| warn!("set_recv_buffer_size(), error: {}", err));

    let _ = sock
        .set_send_buffer_size(16)
        .map_err(|err| warn!("set_send_buffer_size(), error: {}", err));

    loop_fn((sock, 0), move |(sock, i)| {
        Delay::new(Instant::now() + Duration::from_secs(delay))
            .map_err(Error::from)
            .and_then(move |_| {
                tokio::io::write_all(sock, BANNER[i % BANNER.len()])
                    .timeout(Duration::from_secs(timeout))
                    .from_err()
            })
            .and_then(move |(sock, _)| {
                tokio::io::flush(sock)
                    .timeout(Duration::from_secs(timeout))
                    .from_err()
            })
            .map(move |sock| Loop::Continue((sock, i.wrapping_add(1))))
            .or_else(move |err| {
                let connected = NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed) - 1;
                info!(
                    "disconnect, peer: {}, duration: {:.2?}, error: \"{}\", clients: {}",
                    peer,
                    start.elapsed(),
                    err,
                    connected
                );
                Ok(Loop::Break(()))
            })
    })
}

fn main() {
    let opt = Config::from_args();

    let log_level = match opt.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let max_clients = opt.max_clients as usize;
    let delay = opt.delay;
    let timeout = opt.timeout;

    Builder::from_default_env()
        .filter(None, log_level)
        .default_format_timestamp(!opt.disable_timestamps)
        .init();

    let mut rt = Runtime::new()
        .map_err(|err| errx(exitcode::UNAVAILABLE, format!("tokio, error: {:?}", err)))
        .expect("unreachable");

    let startup = Instant::now();

    let listeners: Vec<TcpListener> = opt
        .listen
        .iter()
        .map(|addr| match TcpListener::bind(addr) {
            Ok(listener) => {
                info!("listen, addr: {}", addr);
                listener
            }
            Err(err) => {
                errx(
                    exitcode::OSERR,
                    format!("listen, addr: {}, error: {}", addr, err),
                );
                unreachable!();
            }
        })
        .collect();

    #[cfg(all(unix, feature = "drop_privs"))]
    {
        if opt.privdrop.user.is_some()
            || opt.privdrop.group.is_some()
            || opt.privdrop.chroot.is_some()
        {
            let mut pd = PrivDrop::default();
            if let Some(path) = opt.privdrop.chroot {
                info!("privdrop, chroot: {}", path.display());
                pd = pd.chroot(path);
            }

            if let Some(user) = opt.privdrop.user {
                info!("privdrop, user: {}", user.to_string_lossy());
                pd = pd.user(user);
            }

            if let Some(group) = opt.privdrop.group {
                info!("privdrop, group: {}", group.to_string_lossy());
                pd = pd.group(group);
            }

            pd.apply()
                .unwrap_or_else(|err| errx(71, format!("privdrop, error: {}", err)));

            info!("privdrop, enabled: true");
        } else {
            info!("privdrop, enabled: false");
        }
    }

    #[cfg(all(unix, feature = "sandbox"))]
    {
        let sandboxed = Sandbox::new().sandbox_this_process().is_ok();
        info!("sandbox, enabled: {}", sandboxed);
    }

    info!(
        "start, servers: {}, max_clients: {}, delay: {}s, timeout: {}s",
        listeners.len(),
        opt.max_clients,
        delay,
        timeout
    );

    for listener in listeners.into_iter() {
        let server = listener
            .incoming()
            .sleep_on_error(Duration::from_millis(100))
            .filter_map(|sock| {
                sock.peer_addr()
                    .map_err(|err| error!("peer_addr(), error: {}", err))
                    .map(|peer| (sock, peer))
                    .ok()
            })
            .filter(move |(_sock, peer)| {
                let connected = NUM_CLIENTS.fetch_add(1, Ordering::Relaxed) + 1;

                if connected > max_clients {
                    NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                    info!("reject, peer: {}, clients: {}", peer, connected);
                    false
                } else {
                    info!("connect, peer: {}, clients: {}", peer, connected);
                    true
                }
            })
            .map(move |(sock, peer)| tokio::spawn(tarpit_connection(sock, peer, delay, timeout)))
            .listen(max_clients);

        rt.spawn(server);
    }

    let interrupt = tokio_signal::ctrl_c()
        .flatten_stream()
        .map_err(|error| errx(exitcode::UNAVAILABLE, format!("signal(), error: {}", error)))
        .take(1)
        .for_each(|()| {
            info!("interrupt");
            Ok(())
        });

    rt.block_on(interrupt)
        .map_err(|err| errx(exitcode::UNAVAILABLE, format!("tokio, error: {:?}", err)))
        .expect("unreachable");

    info!(
        "shutdown, uptime: {:.2?}, clients: {}",
        startup.elapsed(),
        NUM_CLIENTS.load(Ordering::Relaxed)
    )
}
