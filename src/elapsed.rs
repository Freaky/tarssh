
use std::fmt;
use std::time::{Duration, Instant};

/// A tiny type for tracking approximate Durations from a known starting point
/// Wraps every 13.6 years, precision of 1 decisecond (100ms)
#[derive(Copy, Clone)]
pub struct Elapsed(u32);

impl From<Instant> for Elapsed {
    fn from(start: Instant) -> Self {
        let duration = start.elapsed();
        Self(duration.as_secs() as u32 * 10 + (duration.subsec_millis() as f32 / 100.0) as u32)
    }
}

impl From<Elapsed> for Duration {
    fn from(elapsed: Elapsed) -> Self {
        Duration::from_millis(elapsed.0 as u64 * 100)
    }
}

impl Elapsed {
    pub fn elapsed(&self, start: Instant) -> Duration {
        start.elapsed() - Duration::from(*self)
    }
}


impl fmt::Debug for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Duration::from(*self).fmt(f)
    }
}