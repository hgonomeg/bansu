use super::{Job, JobSpawnError, JobType};
use crate::AardvarkArgs;
use crate::job::{JobHandleConfiguration, job_handle::JobHandle, job_runner::OutputKind};
use std::{
    time::Duration,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

pub struct AardvarkJob {
    pub args: AardvarkArgs,
}


impl Job for AardvarkJob {
    fn name(&self) -> &'static str {
        "Aardvark"
    }

    fn job_type(&self) -> JobType {
        JobType::Aardvark
    }

    fn timeout_value(&self) -> std::time::Duration {
        if let Some(Ok(tm)) = env::var("BANSU_AARDVARK_TIMEOUT").ok().map(|tm| {
            tm.parse::<u64>().inspect_err(|e| {
                log::error!(
                    "Aardvark timeout could not be parsed: {}. Default value will be used.",
                    e
                )
            })
        }) {
            Duration::from_secs(tm)
        } else {
            Duration::from_secs(4 * 60)
        }
    }

    fn output_filename(&self, workdir_path: &Path, kind: OutputKind) -> Option<PathBuf> {
        match kind {
            OutputKind::JSON => Some(workdir_path.join(format!("{}.json", "todo__name_me"))),
            _ => None, 
        }
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
        //  1. Run acedrg
        // 2. Run aardvark
        todo!()
    }

    fn write_input<'a>(
        &'a self,
        _workdir_path: &'a std::path::Path,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::io::Result<std::path::PathBuf>> + 'a>,
    > {
        // 1. Write input for acedrg
        todo!()
    }

    fn validate_input(&self) -> Result<(), JobSpawnError> {
        todo!()
    }
}
