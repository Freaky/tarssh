# `tarssh` extras

## `tarssh`

A FreeBSD rc script, with full `rc.conf` support.

## `tarssh.service`

An example systemd service file.  The maintainer of `tarssh` is a FreeBSD user,
and cannot directly vouch for it.

## `tarssh_log_stats.rb`

A simple log parser to generate some statistics on the current run of the server,
giving a breakdown of how many clients have been connected and for how long.

Eventually this sort of functionality should be exported via a more structured
means from the server itself.
