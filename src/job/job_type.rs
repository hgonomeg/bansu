pub mod acedrg;
use std::time::Duration;

//use futures_util::Future;

pub trait Job {
    fn name(&self) -> &'static str;
    fn job_type(&self) -> JobType;
    fn timeout_value(&self) -> Duration;
    fn output_filename(&self) -> &'static str;
    fn executable_name(&self) -> &'static str;
    // fn launch(&self);
    // fn write_input(&self) -> Box<dyn Future<Output = std::io::Result<()>>>;

    // todo: input validation
}

pub enum JobType {
    Acedrg,
}
