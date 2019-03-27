use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use env_logger::Builder;
use log::LevelFilter;
use log::{error, info, warn};

use futures::future::{loop_fn, Loop};
use futures::stream::Stream;
use futures::Future;

use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::timer::Delay;

use structopt;
use structopt::StructOpt;

#[cfg(feature = "sandbox")]
use rusty_sandbox::Sandbox;

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
    /// Listen address to bind to
    #[structopt(short = "l", long = "listen", default_value = "0.0.0.0:2222")]
    listen: Vec<SocketAddr>,
    /// Best-effort connection limit
    #[structopt(short = "c", long = "max-clients")]
    max_clients: Option<u32>,
    /// Seconds between responses
    #[structopt(short = "d", long = "delay", default_value = "10")]
    delay: u32,
    /// Verbose level (repeat for more verbosity)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
    /// Disable timestamps in logs
    #[structopt(long = "disable-timestamps")]
    disable_timestamps: bool
}

fn errx<M: AsRef<str>>(code: i32, message: M) {
    error!("{}", message.as_ref());
    std::process::exit(code);
}

fn main() {
    let opt = Config::from_args();

    let log_level = match opt.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let max_clients = opt.max_clients.unwrap_or(u32::max_value()) as usize;
    let delay = u64::from(opt.delay);

    Builder::from_default_env()
        .filter(None, log_level)
        .default_format_timestamp(!opt.disable_timestamps)
        .init();

    let mut rt = Runtime::new()
        .map_err(|err| errx(69, format!("tokio, error: {:?}", err)))
        .expect("unreachable");

    let listeners: Vec<TcpListener> = opt
        .listen
        .iter()
        .map(|addr| match TcpListener::bind(addr) {
            Ok(listener) => {
                info!("listen, addr: {}", addr);
                listener
            }
            Err(err) => {
                errx(71, format!("listen, addr: {}, error: {}", addr, err));
                unreachable!();
            }
        })
        .collect();

    #[cfg(feature = "sandbox")]
    {
        let sandboxed = Sandbox::new().sandbox_this_process().is_ok();
        info!("sandbox mode, enabled: {}", sandboxed);
    }

    info!(
        "start, servers: {}, max_clients: {}, delay: {}s",
        listeners.len(),
        opt.max_clients
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unlimited".to_string()),
        delay
    );

    for listener in listeners.into_iter() {
        let server = listener
            .incoming()
            .map_err(|err| error!("accept(), error: {}", err))
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
            .for_each(move |(sock, peer)| {
                let start = Instant::now();
                let _ = sock
                    .set_recv_buffer_size(1)
                    .map_err(|err| warn!("set_recv_buffer_size(), error: {}", err));

                let _ = sock
                    .set_send_buffer_size(16)
                    .map_err(|err| warn!("set_send_buffer_size(), error: {}", err));

                let tarpit = loop_fn((sock, 0), move |(sock, i)| {
                    Delay::new(Instant::now() + Duration::from_secs(delay))
                        .map_err(|err| {
                            error!("tokio timer, error: {}", err);
                            std::io::Error::new(std::io::ErrorKind::Other, "timer failure")
                        })
                        .and_then(move |_| tokio::io::write_all(sock, BANNER[i % BANNER.len()]))
                        .and_then(|(sock, _)| tokio::io::flush(sock))
                        .map(move |sock| Loop::Continue((sock, i.wrapping_add(1))))
                        .or_else(move |err| {
                            let connected = NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed) - 1;
                            info!(
                                "disconnect, peer: {}, duration: {:.2?}, error: {}, clients: {}",
                                peer,
                                start.elapsed(),
                                err,
                                connected
                            );
                            Ok(Loop::Break(()))
                        })
                });
                tokio::spawn(tarpit)
            });

        rt.spawn(server);
    }

    rt.shutdown_on_idle()
        .wait()
        .map_err(|err| errx(69, format!("tokio, error: {:?}", err)))
        .expect("unreachable");
}
