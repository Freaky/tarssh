
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Timeout,
    TimerFull,
    TimerShutdown,
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::error::Error;
        self.description().fmt(fmt)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Io(err) => err.description(),
            Error::Timeout => "timed out",
            Error::TimerFull => "timer at capacity",
            Error::TimerShutdown => "timer shutdown",
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<tokio::timer::Error> for Error {
    fn from(err: tokio::timer::Error) -> Self {
        if err.is_at_capacity() {
            Error::TimerFull
        } else {
            Error::TimerShutdown
        }
    }
}

impl From<tokio::timer::timeout::Error<std::io::Error>> for Error {
    fn from(err: tokio::timer::timeout::Error<std::io::Error>) -> Self {
        if err.is_elapsed() {
            Error::Timeout
        } else if err.is_inner() {
            Self::from(err.into_inner().expect("IO Error"))
        } else if err.is_timer() {
            Self::from(err.into_timer().expect("Timer error"))
        } else {
            panic!("unhandled timer error: {:?}", err);
        }
    }
}
