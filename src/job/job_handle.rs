use super::docker::ContainerHandle;
use std::{
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output, Stdio},
};
use tokio::process::{Child, Command};

#[derive(Debug)]
pub struct JobProcessConfiguration<'a> {
    pub executable: &'a str,
    pub args: Vec<&'a str>,
    pub working_dir: &'a str,
}

#[derive(Debug)]
pub enum JobHandle {
    Direct(Child),
    Docker(ContainerHandle),
}

impl JobHandle {
    pub async fn new<'a>(cfg: JobProcessConfiguration<'a>) -> anyhow::Result<Self> {
        if let Ok(image) = std::env::var("BANSU_DOCKER") {
            log::info!("Spawning new container for job");
            let handle = ContainerHandle::new(
                &image,
                [&[cfg.executable], cfg.args.as_slice()].concat(),
                cfg.working_dir,
                Some((cfg.working_dir, cfg.working_dir)),
            )
            .await?;
            Ok(Self::Docker(handle))
        } else {
            log::info!("Spawning child process for job");
            let child = Command::new(cfg.executable)
                .current_dir(cfg.working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .args(cfg.args)
                .spawn()?;
            Ok(Self::Direct(child))
        }
    }
    /// This API is currently experimental
    pub async fn join(self) -> anyhow::Result<Output> {
        match self {
            JobHandle::Direct(child) => Ok(child.wait_with_output().await?),
            JobHandle::Docker(handle) => {
                let output = handle.run().await?;
                // The wait API is either terrible or has a serious bug
                // This check below would cause misinterpration of 
                // the failure of processes running in Docker 
                // as failure or the waiting procedure itself
                
                // if let Some(err) = output.exit_info.error.and_then(|e| e.message) {
                //     anyhow::bail!("Error waiting for container: {}", err);
                // }
                Ok(Output {
                    status: ExitStatus::from_raw(output.exit_info.status_code as i32),
                    stdout: output.output.stdout,
                    stderr: output.output.stderr,
                })
            }
        }
    }
}
