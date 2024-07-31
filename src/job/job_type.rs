pub mod acedrg;
use futures_util::Future;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use tokio::process::Child;

pub trait Job {
    fn name(&self) -> &'static str;
    fn job_type(&self) -> JobType;
    fn timeout_value(&self) -> Duration;
    fn output_filename(&self) -> &'static str;
    fn executable_name(&self) -> &'static str;
    fn launch(&self, workdir_path: &Path, input_file_path: &Path) -> std::io::Result<Child>;
    /// Returns path to the input file
    fn write_input<'a>(
        &'a self,
        workdir_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<PathBuf>> + 'a>>;

    // todo: input validation
}

pub enum JobType {
    Acedrg,
}
