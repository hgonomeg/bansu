use futures_util::Future;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

use super::{job_handle::JobHandle, job_runner::OutputKind};

pub mod acedrg;
pub mod servalcat;

pub trait Job: Send {
    fn name(&self) -> &'static str;
    fn job_type(&self) -> JobType;
    fn timeout_value(&self) -> Duration;
    /// Constructs the given type of output filename
    fn output_filename(&self, workdir_path: &Path, kind: OutputKind) -> Option<PathBuf>;
    fn executable_name(&self) -> &'static str;
    // This might need a redesign so that it combines writing input with launching
    fn launch<'a>(
        &'a self,
        workdir_path: &'a Path,
        input_file_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<JobHandle>> + 'a>>;
    /// Returns path to the input file
    fn write_input<'a>(
        &'a self,
        workdir_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<PathBuf>> + 'a>>;

    // todo: input validation
}

pub enum JobType {
    Acedrg,
    Servalcat,
}
