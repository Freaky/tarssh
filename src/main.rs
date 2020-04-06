#![cfg_attr(feature = "nightly", feature(external_doc))]
#![cfg_attr(feature = "nightly", doc(include = "../README.md"))]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::process::exit;

use exitcode;
use futures::stream::StreamExt;
use futures_util::future::FutureExt;
use slog::{info, warn, crit, o, Drain, Logger, Level};
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
    /// JSON logging
    #[structopt(long)]
    log_json: bool,
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


struct FDuration(Duration);

impl std::fmt::Display for FDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2?}", self.0)
    }
}

async fn tarpit_connection(
    mut sock: tokio::net::TcpStream,
    log: Logger,
    delay: Duration,
    time_out: Duration,
) {
    let start = Instant::now();
    sock.set_recv_buffer_size(1)
        .unwrap_or_else(|err| warn!(log, "set_recv_buffer_size()"; "error" => %err));

    sock.set_send_buffer_size(16)
        .unwrap_or_else(|err| warn!(log, "set_send_buffer_size()"; "error" => %err));

    for chunk in BANNER.iter().cycle() {
        delay_for(delay).await;

        let res = timeout(time_out, sock.write_all(chunk.as_bytes()))
            .await
            .unwrap_or_else(|_| Err(std::io::Error::new(std::io::ErrorKind::Other, "timed out")));

        if let Err(err) = res {
            let connected = NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed) - 1;
            info!(
                log, "disconnect";
                "duration" => %FDuration(start.elapsed()), "error" => %err, "clients" => connected
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
        0 => Level::Critical,
        1 => Level::Info,
        2 => Level::Debug,
        _ => Level::Trace,
    };

    let drain = if opt.log_json {
        let drain = slog_json::Json::default(std::io::stdout()).fuse();
        slog_async::Async::new(drain).build().filter_level(log_level).fuse()
    } else {
        let decorator = slog_term::TermDecorator::new().stdout().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        slog_async::Async::new(drain).build().filter_level(log_level).fuse()
    };

    let log = slog::Logger::root(drain, o!());

    let mut rt = tokio::runtime::Builder::new();

    if let Some(threaded) = opt.threads {
        rt.threaded_scheduler();

        if let Some(threads) = threaded {
            let threads = threads.min(512).max(1);
            rt.core_threads(threads);
            info!(log, "init"; "version" => env!("CARGO_PKG_VERSION"), "scheduler" => "threaded", "threads" => threads);
        }
    } else {
        info!(log, "init"; "version" => env!("CARGO_PKG_VERSION"), "scheduler" => "basic");
        rt.basic_scheduler();
    }

    let mut rt = rt
        .enable_all()
        .build()
        .unwrap_or_else(|err| {
            crit!(log, "tokio"; "error" => %err);
            exit(exitcode::UNAVAILABLE);
        });

    let startup = Instant::now();

    let listeners: Vec<_> = opt
        .listen
        .iter()
        .map(
            |addr| match rt.block_on(async { TcpListener::bind(addr).await }) {
                Ok(listener) => {
                    info!(log, "listen"; "addr" => addr);
                    listener
                }
                Err(err) => {
                    crit!(log, "listen"; "addr" => addr, "error" => %err);
                    exit(exitcode::OSERR);
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
                info!(log, "privdrop"; "chroot" => %path.display());
                pd = pd.chroot(path);
            }

            if let Some(user) = opt.privdrop.user {
                info!(log, "privdrop"; "user" => %user.to_string_lossy());
                pd = pd.user(user);
            }

            if let Some(group) = opt.privdrop.group {
                info!(log, "privdrop"; "group" => %group.to_string_lossy());
                pd = pd.group(group);
            }

            pd.apply()
                .unwrap_or_else(|err| {
                    crit!(log, "privdrop"; "error" => %err);
                    exit(exitcode::OSERR);
                });

            info!(log, "privdrop"; "enabled" => true);
        } else {
            info!(log, "privdrop"; "enabled" => false);
        }
    }

    #[cfg(all(unix, feature = "sandbox"))]
    {
        let sandboxed = Sandbox::new().sandbox_this_process().is_ok();
        info!(log, "sandbox"; "enabled" => sandboxed);
    }

    info!(
        log, "start";
        "servers" => listeners.len(), "max_clients" => opt.max_clients,
        "delay" => ?delay, "timeout" => ?timeout
    );

    for mut listener in listeners {
        let log = log.clone();
        let server = async move {
            loop {
                match listener.accept().await {
                    Ok((sock, peer)) => {
                        let connected = NUM_CLIENTS.fetch_add(1, Ordering::Relaxed) + 1;
                        let clog = log.new(o!("peer" => peer));

                        if connected > max_clients {
                            NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                            info!(clog, "reject"; "clients" => connected);
                        } else {
                            info!(clog, "connect"; "clients" => connected);
                            tokio::spawn(tarpit_connection(sock, clog, delay, timeout));
                        }
                    }
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset => (),
                        _ => {
                            let wait = Duration::from_millis(100);
                            warn!(log, "accept"; "error" => %err, "wait" => %FDuration(wait));
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
            crit!(log, "signal()"; "error" => %error);
            exit(exitcode::UNAVAILABLE);
        });
        #[cfg(unix)]
        let term2 = term.recv().into_stream().map(|_| "term");
        #[cfg(unix)]
        let interrupt = futures_util::stream::select(interrupt, term2);

        if let Some(signal) = interrupt.boxed().next().await {
            info!(log, "signal"; "kind" => signal);
        };
    };

    rt.block_on(shutdown);

    info!(log, "shutdown"; "uptime" => %FDuration(startup.elapsed()), "clients" => NUM_CLIENTS.load(Ordering::Relaxed));
}
