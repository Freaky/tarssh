# tarssh

A simple SSH tarpit, similar to [endlessh](https://nullprogram.com/blog/2019/03/22/).

Written in Rust using [Tokio] for async IO and [rusty-sandbox] for optional
sandboxing on FreeBSD (Capsicum), OpenBSD (Pledge) and macOS (Seatbelt).

## Usage

```
-% cargo build --release --features sandbox
-% target/release/tarssh --help
tarssh 0.1.0
Thomas Hurst <tom@hur.st>
A SSH tarpit server

USAGE:
    tarssh [FLAGS] [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Verbose level (repeat for more verbosity)

OPTIONS:
    -d, --delay <delay>                Seconds between responses [default: 10]
    -l, --listen <listen>              Listen address to bind to [default: 0.0.0.0:2222]
    -c, --max-clients <max_clients>    Best-effort connection limit

-% target/release/tarssh -v
[2019-03-26T18:27:50Z INFO  tarssh] listen, addr: 0.0.0.0:2222
[2019-03-26T18:27:50Z INFO  tarssh] sandbox mode, enabled: true
[2019-03-26T18:27:57Z INFO  tarssh] connect, peer: 127.0.0.1:57263, clients: 1
[2019-03-26T18:27:58Z INFO  tarssh] connect, peer: 127.0.0.1:57265, clients: 2
[2019-03-26T18:28:05Z INFO  tarssh] disconnect, peer: 127.0.0.1:57265, duration: 6.04s, error: Broken pipe (os error 32), clients: 1
[2019-03-26T18:28:09Z INFO  tarssh] disconnect, peer: 127.0.0.1:57263, duration: 12.10s, error: Broken pipe (os error 32), clients: 0
```


[Tokio]: https://tokio.rs
[rusty-sandbox]: https://github.com/myfreeweb/rusty-sandbox
