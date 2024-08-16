use std::{
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
};

pub trait JobHandle {
    /// This API is currently experimental
    async fn join(self) -> anyhow::Result<Output>;
}

impl JobHandle for tokio::process::Child {
    async fn join(self) -> anyhow::Result<Output> {
        Ok(self.wait_with_output().await?)
    }
}

impl JobHandle for super::docker::ContainerHandle {
    async fn join(self) -> anyhow::Result<Output> {
        let output = self.run().await?;
        Ok(Output {
            status: ExitStatus::from_raw(output.exit_info.status_code as i32),
            stdout: output.output.stdout,
            stderr: output.output.stderr,
        })
    }
}
