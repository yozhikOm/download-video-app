use std::time::SystemTime;

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
pub struct FixedClock(pub SystemTime);

#[cfg(test)]
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}
