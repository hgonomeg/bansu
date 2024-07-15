use std::{
    env::temp_dir,
    path::{Path, PathBuf},
};
use tokio::{fs, io::AsyncWriteExt};
use uuid::Uuid;

pub struct WorkDir {
    pub path: PathBuf,
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let path = std::mem::take(&mut self.path);
        actix_rt::spawn(async move {
            log::info!("Removing temporary directory {}", path.to_string_lossy());
            let _ = fs::remove_dir_all(path).await;
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
