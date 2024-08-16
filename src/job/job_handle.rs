use super::docker::ContainerHandle;
use std::{
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
};
use tokio::process::Child;

pub enum JobHandle {
    Direct(Child),
    Docker(ContainerHandle),
}

impl JobHandle {
    /// This API is currently experimental
    pub async fn join(self) -> anyhow::Result<Output> {
        match self {
            JobHandle::Direct(child) => Ok(child.wait_with_output().await?),
            JobHandle::Docker(handle) => {
                let output = handle.run().await?;
                if let Some(err) = output.exit_info.error.and_then(|e| e.message) {
                    anyhow::bail!("Error waiting for container: {}", err);
                }
                Ok(Output {
                    status: ExitStatus::from_raw(output.exit_info.status_code as i32),
                    stdout: output.output.stdout,
                    stderr: output.output.stderr,
                })
            }
        }
    }
}
