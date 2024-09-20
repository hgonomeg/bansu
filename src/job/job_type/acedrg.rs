use super::{Job, JobType};
use crate::job::job_handle::{JobHandle, JobProcessConfiguration};
use crate::job::job_runner::OutputKind;
use crate::{utils::dump_string_to_file, AcedrgArgs};
use futures_util::Future;
use std::env;
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
        if let Some(Ok(tm)) = env::var("BANSU_ACEDRG_TIMEOUT")
            .ok()
            .map(|tm| tm.parse::<u64>())
        {
            Duration::from_secs(tm)
        } else {
            Duration::from_secs(2 * 60)
        }
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

    fn validate_input(&self) -> anyhow::Result<()> {
        // to consider: --bsu, --bsl, --asu, --asl, --res (alias to -r), --numInitConf, --multiconf, --numOptmStep
        let allowed_args: [&str; 25] = [
            "-a", "--rechi",
            "-r",
            "-e", "--molgen",
            "-n", "--typeOut",
            "-p", "--coords",
            "-q", "--mdiff",
            "--neu",
            "--keku",
            "--nucl",
            "-u", "--hmo",
            "-z", "--noGeoOpt",
            "-K", "--noProt",
            "-M", "--modifiedPlanes",
            "-k", "-j", "-l"
        ];
        let mut r_arg = false;
        let mut numeric_arg = false;
        for arg in self.args.commandline_args.iter() {
            if !(r_arg || numeric_arg) && !allowed_args.iter().any(|z| z == arg) {
                anyhow::bail!("Input validation failed! Invalid commandline arguments. Supported arguments are: {:?}", &allowed_args);
            }
            if r_arg && !arg.chars().all(|chr| chr.is_alphabetic()) {
                anyhow::bail!("Input validation failed! Non-alphabetic characters used in monomer name (argument of the flag '-r')");
            }
            if numeric_arg && !arg.chars().all(|chr| chr.is_numeric()) {
                anyhow::bail!("Input validation failed! Non-numeric characters used for '-k' or '-j' or '-l'");
            }
            if arg == "-k" || arg == "-j" || arg == "-l" {
                numeric_arg = true;
            } else {
                numeric_arg = false;
            }
            if arg == "-r" {
                r_arg = true;
            } else {
                r_arg = false;
            }
        }
        Ok(())
    }

    fn launch<'a>(
        &'a self,
        workdir_path: &'a Path,
        input_file_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<JobHandle>> + 'a>> {
        // Makes `commandline_args` moveable into the async block without copying
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
