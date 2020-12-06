#![cfg_attr(feature = "nightly", feature(external_doc))]
#![cfg_attr(feature = "nightly", doc(include = "../README.md"))]

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::stream::{self, SelectAll, StreamExt};
use futures_util::future::FutureExt;
use log::LevelFilter;
use log::{error, info, warn};
use retain_mut::RetainMut;
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::time::sleep;

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

static BANNER: &[u8] = "My name is Yon Yonson\r\n\
    I live in Wisconsin\r\n\
    There, the people I meet\r\n\
    As I walk down the street\r\n\
    Say \"Hey, what's your name\"\r\n\
    And I say:\r\n"
    .as_bytes();

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
    delay: std::num::NonZeroU16,
    /// Socket write timeout
    #[structopt(short = "t", long = "timeout", default_value = "30")]
    timeout: u16,
    /// Verbose level (repeat for more verbosity)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
    /// Disable timestamps in logs
    #[structopt(long)]
    disable_log_timestamps: bool,
    /// Disable module name in logs (e.g. "tarssh")
    #[structopt(long)]
    disable_log_ident: bool,
    /// Disable log level in logs (e.g. "info")
    #[structopt(long)]
    disable_log_level: bool,
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
struct Connection {
    sock: TcpStream,  // 40b, includes a local address in an Option with the fd :(
    peer: SocketAddr, // 32b, could shave it down: NonZero u32/u128 enum + u16 port = 18b
    start: Instant,   // 16b, this could just be a u32 seconds or something
    pos: u8,          // 1b, current position within the banner buffer
    failed: u8,       // 1b, number of concurrent times try_write has failed
} // 96 bytes, apparently potential for 70

fn errx<M: AsRef<str>>(code: i32, message: M) -> ! {
    error!("{}", message.as_ref());
    std::process::exit(code);
}

async fn listen_socket(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let sock = match addr {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };

    sock.set_recv_buffer_size(1)
        .unwrap_or_else(|err| warn!("set_recv_buffer_size(), error: {}", err));
    sock.set_send_buffer_size(1)
        .unwrap_or_else(|err| warn!("set_send_buffer_size(), error: {}", err));

    // From mio:
    // On platforms with Berkeley-derived sockets, this allows to quickly
    // rebind a socket, without needing to wait for the OS to clean up the
    // previous one.
    //
    // On Windows, this allows rebinding sockets which are actively in use,
    // which allows “socket hijacking”, so we explicitly don't set it here.
    // https://docs.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse
    #[cfg(not(windows))]
    sock.set_reuseaddr(true)?;

    sock.bind(addr)?;
    sock.listen(1024)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opt = Config::from_args();

    let max_clients = opt.max_clients as usize;
    let delay = Duration::from_secs(u16::from(opt.delay) as u64);
    let timeout = Duration::from_secs(opt.timeout as u64);
    let log_level = match opt.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::Builder::from_default_env()
        .filter(None, log_level)
        .format_timestamp(if opt.disable_log_timestamps {
            None
        } else {
            Some(env_logger::fmt::TimestampPrecision::Millis)
        })
        .format_module_path(!opt.disable_log_ident)
        .format_level(!opt.disable_log_level)
        .init();

    info!("init, version: {}", env!("CARGO_PKG_VERSION"));

    let startup = Instant::now();

    let mut listeners = stream::iter(opt.listen.iter())
        .then(|addr| async move {
            match listen_socket(*addr).await {
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
            }
        })
        .collect::<SelectAll<_>>()
        .await;

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

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<&'static str>();
    let mut shutdown_rx = shutdown_rx.into_stream();
    tokio::spawn(await_shutdown(shutdown_tx));

    let mut num_clients = 0;
    let max_tick = delay.as_secs() as usize;
    let mut last_tick = 0;

    let mut slots: Vec<Vec<Connection>> = Vec::with_capacity(max_tick);
    for _ in 0..max_tick {
        slots.push(vec![]);
    }

    let timer = tokio::time::interval(Duration::from_secs(1));
    let mut ticker = stream::iter(0..max_tick).cycle().zip(timer);

    loop {
        tokio::select! {
            Some(Ok(signal)) = shutdown_rx.next() => {
                info!("{}", signal);
                info!(
                    "shutdown, uptime: {:.2?}, clients: {}",
                    startup.elapsed(),
                    num_clients
                );
                break;
            }
            Some((tick, _)) = ticker.next() => {
                last_tick = tick;
                slots[tick].retain_mut(|mut connection| {
                    let pos = connection.pos as usize;
                    match connection.sock.try_write(&BANNER[pos..pos+1]) {
                        Ok(_) => {
                            connection.pos = (pos as u8 + 1) % BANNER.len() as u8;
                            connection.failed = 0;
                            return true;
                        },
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            connection.failed += 1;
                            if delay * connection.failed as u32 >= timeout {
                                num_clients -= 1;
                                info!(
                                    "disconnect, peer: {}, duration: {:.2?}, error: \"Timed out\", clients: {}",
                                    connection.peer,
                                    connection.start.elapsed(),
                                    num_clients
                                );
                            }
                        },
                        Err(e) => {
                            num_clients -= 1;
                            info!(
                                "disconnect, peer: {}, duration: {:.2?}, error: \"{}\", clients: {}",
                                connection.peer,
                                connection.start.elapsed(),
                                e,
                                num_clients
                            );
                        }
                    }

                    false
                });
            }
            Some(client) = listeners.next(), if num_clients < max_clients => {
                match client {
                    Ok(sock) => {
                        let peer = match sock.peer_addr() {
                            Ok(peer) => peer,
                            Err(e) => {
                                warn!("reject, peer: unknown, error: {:?}", e);
                                continue;
                            }
                        };
                        num_clients += 1;

                        info!("connect, peer: {}, clients: {}", peer, num_clients);
                        let connection = Connection {
                            sock,
                            peer,
                            start: Instant::now(),
                            pos: 0,
                            failed: 0,
                        };
                        slots[last_tick].push(connection);
                    }
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset => (),
                        _ => {
                            let wait = Duration::from_millis(100);
                            warn!("accept, err: {}, wait: {:?}", err, wait);
                            sleep(wait).await;
                        }
                    },
                }
            }
        }
    }
}

async fn await_shutdown(tx: tokio::sync::oneshot::Sender<&'static str>) {
    let interrupt = tokio::signal::ctrl_c().into_stream().map(|_| "interrupt");

    #[cfg(unix)]
    let mut term = signal(SignalKind::terminate())
        .unwrap_or_else(|error| errx(exitcode::UNAVAILABLE, format!("signal(), error: {}", error)));
    #[cfg(unix)]
    let term2 = term.recv().into_stream().map(|_| "terminated");
    #[cfg(unix)]
    let interrupt = futures_util::stream::select(interrupt, term2);

    if let Some(signal) = interrupt.boxed().next().await {
        let _ = tx.send(signal);
    };
}
