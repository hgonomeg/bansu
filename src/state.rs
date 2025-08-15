use std::time::Instant;

pub struct State {
    start_time: Instant,
    pub max_concurrent_jobs: Option<usize>,
    pub version: &'static str,
}

impl State {
    pub fn new(max_jobs: Option<usize>) -> Self {
        Self {
            start_time: Instant::now(),
            max_concurrent_jobs: max_jobs,
            version: env!("CARGO_PKG_VERSION"),
        }
    }
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
