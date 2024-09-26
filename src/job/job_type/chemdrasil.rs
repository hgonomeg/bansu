use super::{Job, JobSpawnError, JobType};
use crate::job::{job_handle::JobHandle, job_runner::OutputKind};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

pub struct Chemdrasil;

impl Job for Chemdrasil {
    fn name(&self) -> &'static str {
        "Chemdrasil"
    }

    fn job_type(&self) -> JobType {
        JobType::Chemdrasil
    }

    fn timeout_value(&self) -> std::time::Duration {
        todo!()
    }

    fn output_filename(&self, _workdir_path: &Path, _kind: OutputKind) -> Option<PathBuf> {
        todo!()
    }

    fn executable_name(&self) -> &'static str {
        "chemdrasil"
    }

    fn launch<'a>(
        &'a self,
        _workdir_path: &'a std::path::Path,
        _input_file_path: &'a std::path::Path,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<JobHandle>> + 'a>> {
        todo!()
    }

    fn write_input<'a>(
        &'a self,
        _workdir_path: &'a std::path::Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::io::Result<std::path::PathBuf>> + 'a>,
    > {
        todo!()
    }

    fn validate_input(&self) -> Result<(), JobSpawnError> {
        todo!()
    }
}
