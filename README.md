[![Cargo](https://img.shields.io/crates/v/tarssh.svg)][crate]
[![Docker](https://img.shields.io/docker/automated/freeky/tarssh.svg)][docker-image]

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
just over a week.

The intent of this is to increase the cost of mass SSH scanning - even clients
that immediately disconnect after the first response are delayed a little, and
that's one less free connection for the next attack.

## Usage

```console
-% cargo install tarssh
-% tarssh --help
tarssh 0.3.0
A SSH tarpit server

USAGE:
    tarssh [FLAGS] [OPTIONS]

FLAGS:
        --disable-timestamps    Disable timestamps in logs
    -h, --help                  Prints help information
    -V, --version               Prints version information
    -v, --verbose               Verbose level (repeat for more verbosity)

OPTIONS:
        --chroot <chroot>              Chroot to this directory
    -d, --delay <delay>                Seconds between responses [default: 10]
    -g, --group <group>                Run as this group
    -l, --listen <listen>...           Listen address(es) to bind to [default: 0.0.0.0:2222]
    -c, --max-clients <max-clients>    Best-effort connection limit [default: 4096]
        --threads <threads>            Use threads, with optional thread count
    -t, --timeout <timeout>            Socket write timeout [default: 30]
    -u, --user <user>                  Run as this user and their primary group


-% tarssh -v --disable-timestamps -l 0.0.0.0:2222 \[::]:2222
[INFO  tarssh] init, version: 0.3.0, scheduler: basic
[INFO  tarssh] listen, addr: 0.0.0.0:2222
[INFO  tarssh] listen, addr: [::]:2222
[INFO  tarssh] privdrop, enabled: false
[INFO  tarssh] sandbox, enabled: true
[INFO  tarssh] start, servers: 2, max_clients: 4096, delay: 10s, timeout: 30s
[INFO  tarssh] connect, peer: 127.0.0.1:39410, clients: 1
[INFO  tarssh] connect, peer: 127.0.0.1:39424, clients: 2
[INFO  tarssh] disconnect, peer: 127.0.0.1:39410, duration: 20.02s, error: "Broken pipe (os error 32)", clients: 1
[INFO  tarssh] disconnect, peer: 127.0.0.1:39424, duration: 20.06s, error: "Broken pipe (os error 32)", clients: 0
^C[INFO  tarssh] interrupt
[INFO  tarssh] shutdown, uptime: 71.50s, clients: 0
```

A dubiously-maintained Docker image is available as [`freeky/tarssh`][docker-image].

```console
-% sudo docker run --network=host freeky/tarssh
Unable to find image 'freeky/tarssh:latest' locally
latest: Pulling from freeky/tarssh
27833a3ba0a5: Pull complete 
1fbf3b23257c: Pull complete 
30379a92040a: Pull complete 
Digest: sha256:a1eccb7dd694753e0d6ea682f5feed2e17dcfc88d817714502b518c381b94298
Status: Downloaded newer image for freeky/tarssh:latest
[2019-04-10T23:02:57Z INFO  tarssh] listen, addr: 0.0.0.0:22
[2019-04-10T23:02:57Z INFO  tarssh] privdrop, chroot: /var/empty
[2019-04-10T23:02:57Z INFO  tarssh] privdrop, user: nobody
[2019-04-10T23:02:57Z INFO  tarssh] privdrop, enabled: true
[2019-04-10T23:02:57Z INFO  tarssh] start, servers: 1, max_clients: 4096, delay: 10s, timeout: 30s
```

[Tokio]: https://tokio.rs
[rusty-sandbox]: https://github.com/myfreeweb/rusty-sandbox
[privdrop]: https://crates.io/crates/privdrop
[crate]: https://crates.io/crates/tarssh
[docker-image]: https://hub.docker.com/r/freeky/tarssh
