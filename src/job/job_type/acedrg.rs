use super::{Job, JobType};
use std::time::Duration;

pub struct AcedrgJob;

impl Job for AcedrgJob {
    fn name(&self) -> &'static str {
        "Acedrg"
    }

    fn timeout_value(&self) -> Duration {
        Duration::from_secs(2 * 60)
    }

    fn output_filename(&self) -> &'static str {
        "acedrg_output"
    }

    fn job_type(&self) -> JobType {
        JobType::Acedrg
    }
}
