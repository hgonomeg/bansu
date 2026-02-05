use base64::prelude::*;
use std::{
    env::temp_dir,
    future::Future,
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};
use tokio::{fs, io::AsyncWriteExt, process::Command};
use uuid::Uuid;

use crate::job::docker::{ContainerHandle, ContainerHandleOutput};

// async fn docker_permission_issue_workaround(image_name: &str, path: PathBuf) {
//     let impl_ = async {
//         let path_str = path
//             .as_os_str()
//             .to_str()
//             .ok_or_else(|| anyhow::anyhow!("Could not convert path to UTF-8"))?;
//         let handle = ContainerHandle::new(
//             image_name,
//             vec!["rm", "-rfv", &path_str],
//             "/tmp",
//             Some((&path_str, &path_str)),
//         )
//         .await
//         .with_context(|| anyhow::anyhow!("Could not spawn container"))?;
//         actix_rt::task::yield_now().await;
//         let _output = handle.run().await?;
//         actix_rt::task::yield_now().await;
//         drop(handle);
//         actix_rt::task::yield_now().await;
//         // Makes no sense
//         // if output.exit_info.status_code != 0 {
//         //     anyhow::bail!("Docker container could not properly execute command: {:?}", output);
//         // } else {
//         //     println!("dbg: {:?}", output);
//         // }
//         // Now we can re-try to delete the folder itself
//         fs::remove_dir_all(&path).await?;
//         anyhow::Ok(())
//     };
//     match impl_.await {
//         Ok(()) => {
//             log::debug!("Docker permission misconfiguration workaround success");
//         }
//         Err(e) => {
//             log::error!("Docker permission misconfiguration workaround failed: {}. Temporary directory \"{}\" could not be removed", e, path.display());
//         }
//     }
// }

pub struct WorkDir {
    pub path: PathBuf,
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let path = std::mem::take(&mut self.path);
        actix_rt::spawn(async move {
            log::debug!("Removing temporary directory {}", path.to_string_lossy());
            if let Err(e) = fs::remove_dir_all(&path).await {
                // if let Ok(image_name) = std::env::var("BANSU_DOCKER") {
                //     if e.kind() == std::io::ErrorKind::PermissionDenied {
                //         log::error!(
                //             "Could not remove temporary directory {} : {}. Your Docker image has misconfigured user and/or permissions,  \
                //             resulting in temporary directories not being able to be deleted. \
                //             Refer to Docker container setup documentation for the fix. A costly and ugly work-around will be employed right now", path.to_string_lossy(),
                //             e
                //         );
                //         actix_rt::task::yield_now().await;
                //         return docker_permission_issue_workaround(&image_name, path).await;
                //     }
                // }
                log::warn!(
                    "Could not remove temporary directory {} : {}",
                    path.to_string_lossy(),
                    e
                );
            }
        });
    }
}

pub async fn mkworkdir() -> std::io::Result<WorkDir> {
    let u = Uuid::new_v4();
    let mut base = temp_dir();
    base.push(format!("bansu-{}", u.to_string()));
    fs::create_dir(&base).await?;
    Ok(WorkDir { path: base })
}

pub async fn dump_string_to_file<P: AsRef<Path>, S: AsRef<str>>(
    filepath: P,
    content: S,
) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        // .truncate(true)
        // .create(true)
        .create_new(true)
        .write(true)
        .open(filepath)
        .await?;
    file.write_all(content.as_ref().as_bytes()).await?;
    Ok(())
}

pub async fn decode_base64_to_file<P: AsRef<Path>, S: AsRef<str>>(
    filepath: P,
    content_base64: S,
) -> std::io::Result<()> {
    let decoded_content = BASE64_STANDARD
        .decode(content_base64.as_ref())
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to decode base64 content: {}", e),
            )
        })?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(filepath)
        .await?;
    file.write_all(&decoded_content).await?;
    Ok(())
}

async fn test_docker_impl(
    image_name: &str,
    commands: Vec<&str>,
) -> anyhow::Result<ContainerHandleOutput> {
    let container = ContainerHandle::new(&image_name, commands, "/tmp", None).await?;
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
        "Output of 'acedrg -v' is \n{}\n{}",
        String::from_utf8_lossy(&acedrg_output.output.stdout),
        String::from_utf8_lossy(&acedrg_output.output.stderr)
    );

    log::info!(
        "Output of 'servalcat -v' is \n{}\n{}",
        String::from_utf8_lossy(&servalcat_output.output.stderr),
        String::from_utf8_lossy(&servalcat_output.output.stdout)
    );
    Ok(())
}

async fn test_dockerless_impl(program: &str, args: &[&str]) -> std::io::Result<Output> {
    Command::new(program).args(args).output().await
}

pub async fn test_dockerless() -> anyhow::Result<()> {
    let (acedrg_res, servalcat_res) = tokio::join!(
        test_dockerless_impl("acedrg", &["-v"]),
        test_dockerless_impl("servalcat", &["-v"])
    );
    let acedrg_output = acedrg_res?;
    let servalcat_output = servalcat_res?;

    log::info!(
        "Output of 'acedrg -v' is \n{}\n{}",
        String::from_utf8_lossy(&acedrg_output.stdout),
        String::from_utf8_lossy(&acedrg_output.stderr)
    );

    if !acedrg_output.status.success() {
        anyhow::bail!("acedrg exited with an error");
    }

    log::info!(
        "Output of 'servalcat -v' is \n{}\n{}",
        String::from_utf8_lossy(&servalcat_output.stderr),
        String::from_utf8_lossy(&servalcat_output.stdout)
    );

    if !servalcat_output.status.success() {
        anyhow::bail!("servalcat exited with an error");
    }
    Ok(())
}

pub fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let begin = tokio::time::Instant::now();
    let r = f();
    let end = tokio::time::Instant::now();
    (r, end - begin)
}

pub async fn measure_time_async<Fut, R>(f: Fut) -> (R, Duration)
where
    Fut: Future<Output = R>,
{
    let begin = tokio::time::Instant::now();
    let r = f.await;
    let end = tokio::time::Instant::now();
    (r, end - begin)
}
