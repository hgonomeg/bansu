use sea_orm::DatabaseConnection;
use std::time::Instant;

pub struct State {
    start_time: Instant,
    pub max_concurrent_jobs: Option<usize>,
    pub version: &'static str,
    pub usage_stats_db: Option<DatabaseConnection>,
}

impl State {
    pub fn new(max_jobs: Option<usize>, usage_stats_db: Option<DatabaseConnection>) -> Self {
        Self {
            start_time: Instant::now(),
            max_concurrent_jobs: max_jobs,
            version: env!("CARGO_PKG_VERSION"),
            usage_stats_db,
        }
    }
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
    pub async fn on_usage_stats_db_async<F, T, Fut>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&DatabaseConnection) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        if let Some(db) = &self.usage_stats_db {
            Some(f(db).await)
        } else {
            None
        }
    }
    pub fn on_usage_stats_db<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&DatabaseConnection) -> T,
    {
        if let Some(db) = &self.usage_stats_db {
            Some(f(db))
        } else {
            None
        }
    }
}
