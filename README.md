# tarssh

A simple SSH tarpit, similar to [endlessh](https://nullprogram.com/blog/2019/03/22/).

Written in Rust using [Tokio] for async IO and [rusty-sandbox] for optional
sandboxing on FreeBSD (Capsicum), OpenBSD (Pledge) and macOS (Seatbelt).

## Usage

```
-% cargo build --release
-% target/release/tarssh --help
tarssh 0.1.0
Thomas Hurst <tom@hur.st>
A SSH tarpit server

USAGE:
    tarssh [FLAGS] [OPTIONS]

FLAGS:
        --disable-timestamps    Disable timestamps in logs
    -h, --help                  Prints help information
    -V, --version               Prints version information
    -v, --verbose               Verbose level (repeat for more verbosity)

OPTIONS:
    -d, --delay <delay>                Seconds between responses [default: 10]
    -l, --listen <listen>...           Listen address(es) to bind to [default: 0.0.0.0:2222]
    -c, --max-clients <max_clients>    Best-effort connection limit

-% target/release/tarssh -v --disable-timestamps -l 10.0.1.1:2222 127.0.0.1:2222
[INFO  tarssh] listen, addr: 10.0.1.1:2222
[INFO  tarssh] listen, addr: 127.0.0.1:2222
[INFO  tarssh] sandbox mode, enabled: true
[INFO  tarssh] start, servers: 2, max_clients: unlimited, delay: 10s
[INFO  tarssh] connect, peer: 127.0.0.1:41989, clients: 1
[INFO  tarssh] connect, peer: 127.0.0.1:42001, clients: 2
[INFO  tarssh] disconnect, peer: 127.0.0.1:41989, duration: 20.19s, error: Broken pipe (os error 32), clients: 1
[INFO  tarssh] disconnect, peer: 127.0.0.1:42001, duration: 30.08s, error: Broken pipe (os error 32), clients: 0
```


[Tokio]: https://tokio.rs
[rusty-sandbox]: https://github.com/myfreeweb/rusty-sandbox
