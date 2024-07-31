use super::{Job, JobType};
use crate::job::job_runner::OutputKind;
use crate::{utils::dump_string_to_file, AcedrgArgs};
use futures_util::Future;
use std::process::Stdio;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use tokio::process::{Child, Command};

const ACEDRG_OUTPUT_FILENAME: &'static str = "acedrg_output";

pub struct AcedrgJob {
    pub args: AcedrgArgs,
}

impl Job for AcedrgJob {
    fn name(&self) -> &'static str {
        "Acedrg"
    }

    fn timeout_value(&self) -> Duration {
        Duration::from_secs(2 * 60)
    }

    fn output_filename(&self, workdir_path: &Path, kind: OutputKind) -> Option<PathBuf> {
        match kind {
            OutputKind::CIF => Some(workdir_path.join(format!("{}.cif", ACEDRG_OUTPUT_FILENAME))),
            // _ => None
        }
    }

    fn job_type(&self) -> JobType {
        JobType::Acedrg
    }

    fn executable_name(&self) -> &'static str {
        "acedrg"
    }

    fn launch(&self, workdir_path: &Path, input_file_path: &Path) -> std::io::Result<Child> {
        Command::new(self.executable_name())
            .current_dir(workdir_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-i")
            .arg(input_file_path)
            // TODO: SANITIZE INPUT!
            .args(self.args.commandline_args.clone())
            .arg("-o")
            .arg(ACEDRG_OUTPUT_FILENAME)
            .spawn()
    }

    fn write_input<'a>(
        &'a self,
        workdir_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<PathBuf>> + 'a>> {
        let smiles_file_path = workdir_path.join("acedrg_smiles_input");
        let input_content = &self.args.smiles;
        Box::pin(async move {
            dump_string_to_file(&smiles_file_path, input_content)
                .await
                .map(|_nothing| smiles_file_path)
        })
    }
}
