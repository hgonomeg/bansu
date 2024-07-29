pub mod acedrg;
use std::time::Duration;

pub trait Job {
    fn name(&self) -> &'static str;
    fn job_type(&self) -> JobType;
    fn timeout_value(&self) -> Duration;
    fn output_filename(&self) -> &'static str;
    // todo: input validation
}

pub enum JobType {
    Acedrg,
}
