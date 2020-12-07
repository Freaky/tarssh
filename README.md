[![Cargo](https://img.shields.io/crates/v/tarssh.svg)][crate]

# tarssh

A simple SSH tarpit, similar to [endlessh](https://nullprogram.com/blog/2019/03/22/).

As per [RFC 4253](https://tools.ietf.org/html/rfc4253#page-4):

```txt
   The server MAY send other lines of data before sending the version
   string.  Each line SHOULD be terminated by a Carriage Return and Line
   Feed.  Such lines MUST NOT begin with "SSH-", and SHOULD be encoded
   in ISO-10646 UTF-8 [RFC3629] (language is not specified).  Clients
   MUST be able to process such lines.
```

In other words, you can fool SSH clients into waiting an extremely long time for
a SSH handshake to even begin simply by waffling on endlessly.  My high score is
just over a fortnight.

The intent of this is to increase the cost of mass SSH scanning - even clients
that immediately disconnect after the first response are delayed a little, and
that's one less free connection for the next attack.

## Usage

```console
-% cargo install tarssh
-% tarssh --help
tarssh 0.6.0
A SSH tarpit server

USAGE:
    tarssh [FLAGS] [OPTIONS]

FLAGS:
        --disable-log-ident         Disable module name in logs (e.g. "tarssh")
        --disable-log-level         Disable log level in logs (e.g. "info")
        --disable-log-timestamps    Disable timestamps in logs
    -h, --help                      Prints help information
    -V, --version                   Prints version information
    -v, --verbose                   Verbose level (repeat for more verbosity)

OPTIONS:
        --chroot <chroot>              Chroot to this directory
    -d, --delay <delay>                Seconds between responses [default: 10]
    -g, --group <group>                Run as this group
    -l, --listen <listen>...           Listen address(es) to bind to [default: 0.0.0.0:2222]
    -c, --max-clients <max-clients>    Best-effort connection limit [default: 4096]
    -t, --timeout <timeout>            Socket write timeout [default: 30]
    -u, --user <user>                  Run as this user and their primary group

-% tarssh -v --disable-log-timestamps --disable-log-ident -l 0.0.0.0:2222 \[::]:2222
[INFO ] init, pid: 27344, version: 0.6.0
[INFO ] listen, addr: 0.0.0.0:2222
[INFO ] listen, addr: [::]:2222
[INFO ] privdrop, enabled: false
[INFO ] sandbox, enabled: true
[INFO ] start, servers: 2, max_clients: 4096, delay: 10s, timeout: 30s
[INFO ] connect, peer: 127.0.0.1:61986, clients: 1
[INFO ] connect, peer: 127.0.0.1:61988, clients: 2
load: 1.05  cmd: tarssh 27344 [kqread] 6.92r 0.00u 0.00s 0% 4512k
[INFO ] info, pid: 27344, signal: INFO, uptime: 6.92s, clients: 2, total: 2, bytes: 0
[INFO ] disconnect, peer: 127.0.0.1:61986, duration: 19.80s, bytes: 24, error: "Broken pipe (os error 32)", clients: 1
[INFO ] disconnect, peer: 127.0.0.1:61988, duration: 19.62s, bytes: 24, error: "Broken pipe (os error 32)", clients: 0
^C[INFO ] shutdown, pid: 27344, signal: INT, uptime: 25.39s, clients: 0, total: 2, bytes: 48
```

The `info` line is generated using a BSD `SIGINFO` signal - `SIGHUP` is also
supported for Unix platforms lacking this.


[Tokio]: https://tokio.rs
[rusty-sandbox]: https://github.com/myfreeweb/rusty-sandbox
[privdrop]: https://crates.io/crates/privdrop
[crate]: https://crates.io/crates/tarssh
