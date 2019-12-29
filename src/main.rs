#![cfg_attr(feature = "nightly", feature(external_doc))]
#![cfg_attr(feature = "nightly", doc(include = "../README.md"))]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use env_logger;
use exitcode;
use futures::stream::StreamExt;
use futures_util::future::FutureExt;
use log::LevelFilter;
use log::{error, info, warn};
use structopt;
use structopt::StructOpt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::{delay_for, timeout};

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

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
    /// Use threads, with optional thread count
    #[structopt(long = "threads")]
    #[allow(clippy::option_option)]
    threads: Option<Option<usize>>,
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

fn errx<M: AsRef<str>>(code: i32, message: M) -> ! {
    error!("{}", message.as_ref());
    std::process::exit(code);
}

async fn tarpit_connection(
    mut sock: tokio::net::TcpStream,
    peer: SocketAddr,
    delay: Duration,
    time_out: Duration,
) {
    let start = Instant::now();
    sock.set_recv_buffer_size(1)
        .unwrap_or_else(|err| warn!("set_recv_buffer_size(), error: {}", err));

    sock.set_send_buffer_size(16)
        .unwrap_or_else(|err| warn!("set_send_buffer_size(), error: {}", err));

    for chunk in BANNER.iter().cycle() {
        delay_for(delay).await;

        let res = timeout(time_out, sock.write_all(chunk.as_bytes()))
            .await
            .unwrap_or_else(|_| Err(std::io::Error::new(std::io::ErrorKind::Other, "timed out")));

        if let Err(err) = res {
            let connected = NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed) - 1;
            info!(
                "disconnect, peer: {}, duration: {:.2?}, error: \"{}\", clients: {}",
                peer,
                start.elapsed(),
                err,
                connected
            );
            break;
        }
    }
}

fn main() {
    let opt = Config::from_args();

    let max_clients = opt.max_clients as usize;
    let delay = Duration::from_secs(opt.delay);
    let timeout = Duration::from_secs(opt.timeout);
    let log_level = match opt.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::Builder::from_default_env()
        .filter(None, log_level)
        .format_timestamp(if opt.disable_timestamps {
            None
        } else {
            Some(env_logger::fmt::TimestampPrecision::Millis)
        })
        .init();

    let mut rt = tokio::runtime::Builder::new();
    let mut scheduler;

    if let Some(threaded) = opt.threads {
        rt.threaded_scheduler();

        scheduler = "threaded".to_string();

        if let Some(threads) = threaded {
            let threads = threads.min(512).max(1);
            rt.core_threads(threads);
            scheduler = format!("threaded, threads: {}", threads);
        }
    } else {
        scheduler = "basic".to_string();
        rt.basic_scheduler();
    }

    info!("init, version: {}, scheduler: {}", env!("CARGO_PKG_VERSION"), scheduler);

    let mut rt = rt
        .enable_all()
        .build()
        .unwrap_or_else(|err| errx(exitcode::UNAVAILABLE, format!("tokio, error: {:?}", err)));

    let startup = Instant::now();

    let listeners: Vec<_> = opt
        .listen
        .iter()
        .map(
            |addr| match rt.block_on(async { TcpListener::bind(addr).await }) {
                Ok(listener) => {
                    info!("listen, addr: {}", addr);
                    listener
                }
                Err(err) => {
                    errx(
                        exitcode::OSERR,
                        format!("listen, addr: {}, error: {}", addr, err),
                    );
                }
            },
        )
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
                .unwrap_or_else(|err| errx(exitcode::OSERR, format!("privdrop, error: {}", err)));

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
        delay.as_secs(),
        timeout.as_secs()
    );

    for mut listener in listeners {
        let server = async move {
            loop {
                match listener.accept().await {
                    Ok((sock, peer)) => {
                        let connected = NUM_CLIENTS.fetch_add(1, Ordering::Relaxed) + 1;

                        if connected > max_clients {
                            NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                            info!("reject, peer: {}, clients: {}", peer, connected);
                        } else {
                            info!("connect, peer: {}, clients: {}", peer, connected);
                            tokio::spawn(tarpit_connection(sock, peer, delay, timeout));
                        }
                    }
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset => (),
                        _ => {
                            let wait = Duration::from_millis(100);
                            warn!("accept, err: {}, wait: {:?}", err, wait);
                            delay_for(wait).await;
                        }
                    },
                }
            }
        };

        rt.spawn(server);
    }

    let shutdown = async {
        let interrupt = tokio::signal::ctrl_c().into_stream().map(|_| "interrupt");

        #[cfg(unix)]
        let mut term = signal(SignalKind::terminate()).unwrap_or_else(|error| {
            errx(exitcode::UNAVAILABLE, format!("signal(), error: {}", error))
        });
        #[cfg(unix)]
        let term2 = term.recv().into_stream().map(|_| "terminated");
        #[cfg(unix)]
        let interrupt = futures_util::stream::select(interrupt, term2);

        if let Some(signal) = interrupt.boxed().next().await {
            info!("{}", signal);
        };
    };

    rt.block_on(shutdown);

    info!(
        "shutdown, uptime: {:.2?}, clients: {}",
        startup.elapsed(),
        NUM_CLIENTS.load(Ordering::Relaxed)
    )
}
