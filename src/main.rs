use std::env;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::future::loop_fn;
use futures::stream::Stream;
use futures::Future;
use rand::{thread_rng, Rng};
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::timer::Delay;

fn main() -> Result<(), Box<std::error::Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:2222".to_string());
    let addr = addr.parse::<SocketAddr>()?;
    let listener = TcpListener::bind(&addr)?;
    eprintln!("Listening on: {}", addr);

    let server = listener
        .incoming()
        .map_err(|e| eprintln!("accept(): {:?}", e))
        .for_each(|sock| {
            eprintln!(
                "Connection: {} -> {}",
                sock.local_addr().expect("local addr"),
                sock.peer_addr().expect("peer address")
            );

            // Minimise receive buffer size, slows clients and minimises resource use
            let _ = sock.set_recv_buffer_size(1)
                .map_err(|err| eprintln!("set_recv_buffer_size(): {:?}", err));

            let tarpit = loop_fn(sock, move |sock| {
                Delay::new(Instant::now() + Duration::from_secs(thread_rng().gen_range(1, 10)))
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "timer fail"))
                    .and_then(move |_| {
                        tokio::io::write_all(sock, format!("{:x}\r\n", thread_rng().gen::<u32>()))
                    })
                    .map(|(sock, _)| future::Loop::Continue(sock))
                    .or_else(|err| {
                        eprintln!("Connection closed {:?}", err);
                        Ok(future::Loop::Break(()))
                    })
            });
            tokio::spawn(tarpit)
        });

    tokio::run(server);

    Ok(())
}
