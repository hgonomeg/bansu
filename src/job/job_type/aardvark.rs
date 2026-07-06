use super::{Job, JobSpawnError, JobType};
use crate::AardvarkArgs;
use crate::job::{JobHandleConfiguration, job_handle::JobHandle, job_runner::OutputKind};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

pub struct AardvarkJob {
    pub args: AardvarkArgs,
}

pub struct Aardvark;

impl Job for Aardvark {
    fn name(&self) -> &'static str {
        "Aardvark"
    }

    fn job_type(&self) -> JobType {
        JobType::Aardvark
    }

    fn timeout_value(&self) -> std::time::Duration {
        todo!()
    }

    fn output_filename(&self, _workdir_path: &Path, _kind: OutputKind) -> Option<PathBuf> {
        todo!()
    }

    fn executable_name(&self) -> &'static str {
        "aardvark"
    }

    fn launch<'a>(
        &'a self,
        _job_handle_configuration: JobHandleConfiguration,
        _workdir_path: &'a Path,
        _input_file_path: &'a Path,
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
