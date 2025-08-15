use std::time::Instant;

pub struct State {
    start_time: Instant,
    pub version: &'static str,
}

impl State {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION"),
        }
    }
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
