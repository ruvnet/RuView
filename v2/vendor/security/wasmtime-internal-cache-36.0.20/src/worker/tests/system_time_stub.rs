use std::sync::LazyLock;
use std::time::{Duration, SystemTime, SystemTimeError};

pub static NOW: LazyLock<SystemTime> = LazyLock::new(SystemTime::now);

#[derive(PartialOrd, PartialEq, Ord, Eq)]
pub struct SystemTimeStub(SystemTime);

impl SystemTimeStub {
    pub fn now() -> Self {
        Self(*NOW)
    }

    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration).map(|t| t.into())
    }

    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        self.0.duration_since(earlier)
    }
}

impl From<SystemTime> for SystemTimeStub {
    fn from(time: SystemTime) -> Self {
        Self(time)
    }
}
