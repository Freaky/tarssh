use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use exitcode;

use env_logger;
use log::{error, info, warn};

use futures::future::{Loop, loop_fn};
use futures::stream::Stream;
use futures::Future;

use tokio::net::TcpListener;
use tokio::timer::Delay;

use rand::{thread_rng, Rng};

static NUM_CLIENTS: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature="capsicum")]
use capsicum;

fn errx<M: AsRef<str>>(code: i32, message: M) {
    error!("{}", message.as_ref());
    std::process::exit(code);
}

fn main() {
    env_logger::init();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:2222".to_string())
        .parse::<SocketAddr>()
        .map_err(|_| errx(exitcode::USAGE, "Error parsing listen address"))
        .expect("unreachable");

    let listener = TcpListener::bind(&addr)
        .map_err(|err| errx(exitcode::OSERR, format!("bind(), error: {}", err)))
        .expect("unreachable");

    info!("listen, addr: {}", addr);

    #[cfg(feature="capsicum")]
    {
        let _ = capsicum::enter();
        info!("capsicum sandbox, enabled: {}", capsicum::sandboxed());
    }

    let server = listener
        .incoming()
        .map_err(|err| error!("accept(), error: {}", err))
        .filter_map(|sock| {
            sock.peer_addr()
                .map_err(|err| error!("peer_addr(), error: {}", err))
                .map(|peer| (sock, peer))
                .ok()
        })
        .for_each(|(sock, peer)| {
            let connected = NUM_CLIENTS.fetch_add(1, Ordering::Relaxed);

            info!("connect, peer: {}, clients: {}", peer, connected + 1);

            let start = Instant::now();
            let _ = sock
                .set_recv_buffer_size(1)
                .map_err(|err| warn!("set_recv_buffer_size(), error: {}", err));

            let tarpit = loop_fn(sock, move |sock| {
                Delay::new(Instant::now() + Duration::from_secs(10))
                    .map_err(|err| {
                        error!("tokio timer, error: {}", err);
                        std::io::Error::new(std::io::ErrorKind::Other, "timer failure")
                    })
                    .and_then(move |_| {
                        tokio::io::write_all(sock, format!("{:x}\r\n", thread_rng().gen::<u32>()))
                    })
                    .map(|(sock, _)| Loop::Continue(sock))
                    .or_else(move |err| {
                        let connected = NUM_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                        info!(
                            "disconnect, peer: {}, duration: {:.2?}, error: {}, clients: {}",
                            peer,
                            start.elapsed(),
                            err,
                            connected - 1
                        );
                        Ok(Loop::Break(()))
                    })
            });
            tokio::spawn(tarpit)
        });

    tokio::run(server);
}
