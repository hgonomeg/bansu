use anyhow::Context;
use bollard::{
    container::{
        AttachContainerOptions, CreateContainerOptions, LogOutput, StartContainerOptions,
        WaitContainerOptions,
    },
    secret::ContainerWaitResponse,
    Docker,
};
use futures_util::StreamExt;
use uuid::Uuid;

async fn test_docker_impl(
    image_name: &str,
    commands: Vec<&str>,
) -> anyhow::Result<ContainerHandleOutput> {
    let container = ContainerHandle::new(&image_name, commands, "/").await?;
    let output = container.run().await?;
    if output.exit_info.status_code != 0 {
        anyhow::bail!(
            "The container returned with {} exit code. {}",
            output.exit_info.status_code,
            output
                .exit_info
                .error
                .and_then(|e| e.message)
                .unwrap_or_default()
        );
    }
    Ok(output)
}

pub async fn test_docker(image_name: &str) -> anyhow::Result<()> {
    let (acedrg_res, servalcat_res) = tokio::join!(
        test_docker_impl(image_name, vec!["acedrg", "-v"]),
        test_docker_impl(image_name, vec!["servalcat", "-v"])
    );
    let acedrg_output = acedrg_res?;
    let servalcat_output = servalcat_res?;

    log::info!(
        "Output of 'acedrg -v' is {}\n{}",
        String::from_utf8_lossy(&acedrg_output.output.stdout),
        String::from_utf8_lossy(&acedrg_output.output.stderr)
    );

    log::info!(
        "Output of 'servalcat -v' is {}\n{}",
        String::from_utf8_lossy(&servalcat_output.output.stderr),
        String::from_utf8_lossy(&servalcat_output.output.stdout)
    );
    Ok(())
}
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
            .with_context(|| "Could not start Docker container.")?;

        let mut exit_info = None;
        while let Some(res) = wait_stream.next().await {
            exit_info = Some(res.with_context(|| "Error waiting for container.")?);
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
