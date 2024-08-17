use anyhow::Context;
use bollard::{
    container::{
        AttachContainerOptions, CreateContainerOptions, LogOutput, StartContainerOptions,
        WaitContainerOptions,
    },
    secret::{ContainerWaitExitError, ContainerWaitResponse, HostConfig, Mount, MountTypeEnum},
    Docker,
};
use futures_util::StreamExt;
use uuid::Uuid;

#[derive(Debug)]
pub struct ContainerHandle {
    docker: Docker,
    pub name: String,
    pub id: String,
}

#[derive(Debug, Default)]
pub struct ContainerLogs {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct ContainerHandleOutput {
    pub exit_info: ContainerWaitResponse,
    pub output: ContainerLogs,
}

impl ContainerHandle {
    pub async fn new(
        image_name: &str,
        command: Vec<&str>,
        local_working_dir: &str,
        mount_bind: Option<(&str, &str)>,
    ) -> anyhow::Result<Self> {
        log::debug!("Connecting to Docker...");
        let docker = tokio::task::spawn_blocking(|| Docker::connect_with_defaults())
            .await
            .unwrap()
            .with_context(|| "Could not connect to Docker")?;

        let u = Uuid::new_v4();
        let container_name = format!("bansu-worker-{}", u.to_string());
        let config = bollard::container::Config {
            cmd: Some(command),
            image: Some(image_name),
            working_dir: Some(local_working_dir),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            // We are probably gonna run as root anyway.
            // Running as non-root currently causes permission issues 
            // while deleting temporary files
            // user: Some("bansu"),
            host_config: mount_bind.map(|(src, dst)| HostConfig {
                mounts: Some(vec![Mount {
                    source: Some(src.to_string()),
                    target: Some(dst.to_string()),
                    typ: Some(MountTypeEnum::BIND),
                    consistency: Some("default".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        log::debug!("Creating container \"{}\"", &container_name);
        let opts = CreateContainerOptions {
            name: container_name.clone(),
            platform: None::<String>,
        };
        let container = docker
            .create_container(Some(opts), config)
            .await
            .with_context(|| "Could not create Docker container")?;

        log::info!(
            "Created Docker container with id={} name={}",
            &container.id,
            &container_name
        );

        Ok(Self {
            docker,
            name: container_name,
            id: container.id,
        })
    }

    pub async fn run(&self) -> anyhow::Result<ContainerHandleOutput> {
        let mut wait_stream = self.docker.wait_container(
            &self.id,
            Some(WaitContainerOptions {
                condition: "not-running",
            }),
        );
        log::debug!("Launching Docker container {}", &self.id);

        let mut grasp = self
            .docker
            .attach_container(
                &self.id,
                Some(AttachContainerOptions::<String> {
                    stdin: Some(false),
                    stdout: Some(true),
                    stderr: Some(true),
                    stream: Some(true),
                    logs: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .with_context(|| "Could not attach to Docker container")?;

        let log_worker = tokio::task::spawn(async move {
            let mut output = ContainerLogs::default();
            while let Some(log_output_res) = grasp.output.next().await {
                match log_output_res {
                    Ok(LogOutput::StdErr { message }) => {
                        output.stderr.extend_from_slice(&message);
                    }
                    Ok(LogOutput::StdOut { message }) => {
                        output.stdout.extend_from_slice(&message);
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    _ => {}
                }
            }
            return Ok(output);
        });

        self.docker
            .start_container(&self.id, None::<StartContainerOptions<String>>)
            .await
            .with_context(|| "Could not start Docker container")?;

        let mut exit_info = None;
        while let Some(res) = wait_stream.next().await {
            // the wait API is terrible
            // or has a bug
            match res {
                Ok(ei) => {
                    exit_info = Some(ei);
                }
                // This uglyness prevents misinterpreting the failure of processes
                // running in Docker as failure or the waiting procedure itself
                Err(bollard::errors::Error::DockerContainerWaitError { error, code }) => {
                    // log::warn!("Error {:#?}", e);
                    exit_info = Some(ContainerWaitResponse {
                        status_code: code,
                        error: Some(ContainerWaitExitError {
                            message: Some(error),
                        }),
                    });
                }
                Err(e) => {
                    anyhow::bail!("Error while waiting for container: {:#?}", e);
                }
            }
        }

        let log_worker_output = log_worker
            .await
            .unwrap()
            .with_context(|| "Failed to collect logs from the container")?;

        log::info!("Finished running in Docker container {}", &self.id);

        Ok(ContainerHandleOutput {
            exit_info: exit_info.unwrap(),
            output: log_worker_output,
        })
    }
}

impl Drop for ContainerHandle {
    fn drop(&mut self) {
        let id = std::mem::take(&mut self.id);
        let d = self.docker.clone();
        actix_rt::spawn(async move {
            log::info!("Removing container {}", &id);
            if let Err(e) = d.remove_container(&id, None).await {
                log::error!("Could not remove container: {}", e);
            }
        });
    }
}
