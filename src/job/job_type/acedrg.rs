use super::{Job, JobType};
use crate::job::job_handle::{JobHandle, JobProcessConfiguration};
use crate::job::job_runner::OutputKind;
use crate::{utils::dump_string_to_file, AcedrgArgs};
use futures_util::Future;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

const ACEDRG_OUTPUT_FILENAME: &'static str = "acedrg_output";

pub struct AcedrgJob {
    pub args: AcedrgArgs,
}

impl Job for AcedrgJob {
    fn name(&self) -> &'static str {
        "Acedrg"
    }

    fn timeout_value(&self) -> Duration {
        Duration::from_secs(4 * 60)
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

    fn launch<'a>(
        &'a self,
        workdir_path: &'a Path,
        input_file_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<JobHandle>> + 'a>> {
        // TODO: SANITIZE INPUT!
        let commandline_args = self.args.commandline_args.iter().map(|z| z.as_str());
        Box::pin(async move {
            let mut args = vec![
                "-i",
                input_file_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Could not convert input_file_path to UTF-8"))?,
            ];
            args.extend(commandline_args);
            args.extend_from_slice(&["-o", ACEDRG_OUTPUT_FILENAME]);

            JobHandle::new(JobProcessConfiguration {
                executable: self.executable_name(),
                args,
                working_dir: workdir_path
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Could not convert workdir_path to UTF-8"))?,
            })
            .await
        })
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
