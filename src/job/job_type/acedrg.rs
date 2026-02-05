use super::{Job, JobSpawnError, JobType};
use crate::job::job_handle::{JobHandle, JobHandleConfiguration, JobProcessConfiguration};
use crate::job::job_runner::OutputKind;
use crate::{
    AcedrgArgs,
    utils::{decode_base64_to_file, dump_string_to_file},
};
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
        if let Some(Ok(tm)) = env::var("BANSU_ACEDRG_TIMEOUT").ok().map(|tm| {
            tm.parse::<u64>().inspect_err(|e| {
                log::error!(
                    "Acedrg timeout could not be parsed: {}. Default value will be used.",
                    e
                )
            })
        }) {
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

    fn validate_input(&self) -> Result<(), JobSpawnError> {
        if self.args.smiles.is_none() && self.args.input_mmcif_base64.is_none() {
            return Err(JobSpawnError::InputValidation(
                "Input validation failed! Either a SMILES string or an mmCIF file (base64-encoded) must be provided.".to_string(),
            ));
        }
        if self.args.smiles.is_some() && self.args.input_mmcif_base64.is_some() {
            return Err(JobSpawnError::InputValidation(
                "Input validation failed! Both SMILES string and mmCIF file were provided. Use only one of them at a time.".to_string(),
            ));
        }
        // to consider: --bsu, --bsl, --asu, --asl, --res (alias to -r), --numInitConf, --multiconf, --numOptmStep
        let allowed_args: [&str; 25] = [
            "-a",
            "--rechi",
            "-r",
            "-e",
            "--molgen",
            "-n",
            "--typeOut",
            "-p",
            "--coords",
            "-q",
            "--mdiff",
            "--neu",
            "--keku",
            "--nucl",
            "-u",
            "--hmo",
            "-z",
            "--noGeoOpt",
            "-K",
            "--noProt",
            "-M",
            "--modifiedPlanes",
            "-k",
            "-j",
            "-l",
        ];
        let mut r_arg = false;
        let mut numeric_arg = false;
        for arg in self.args.commandline_args.iter() {
            if !(r_arg || numeric_arg) && !allowed_args.iter().any(|z| z == arg) {
                return Err(JobSpawnError::InputValidation(format!(
                    "Input validation failed! Invalid commandline arguments. Supported arguments are: {:?}",
                    &allowed_args
                )));
            }
            if r_arg && !arg.chars().all(|chr| chr.is_alphabetic()) {
                return Err(JobSpawnError::InputValidation(format!(
                    "Input validation failed! Non-alphabetic characters used in monomer name (argument of the flag '-r')"
                )));
            }
            if numeric_arg && !arg.chars().all(|chr| chr.is_numeric()) {
                return Err(JobSpawnError::InputValidation(format!(
                    "Input validation failed! Non-numeric characters used for '-k' or '-j' or '-l'"
                )));
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
        job_handle_configuration: JobHandleConfiguration,
        workdir_path: &'a Path,
        input_file_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<JobHandle>> + 'a>> {
        // Makes `commandline_args` moveable into the async block without copying
        let commandline_args = self.args.commandline_args.iter().map(|z| z.as_str());
        let smiles_or_mmcif_mode = if self.args.smiles.is_some() {
            true
        } else {
            false
        };
        Box::pin(async move {
            let mut args = vec![
                if smiles_or_mmcif_mode { "-i" } else { "-c" },
                input_file_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Could not convert input_file_path to UTF-8"))?,
            ];
            args.extend(commandline_args);
            args.extend_from_slice(&["-o", ACEDRG_OUTPUT_FILENAME]);

            JobHandle::new(
                JobProcessConfiguration {
                    executable: self.executable_name(),
                    args,
                    working_dir: workdir_path.as_os_str().to_str().ok_or_else(|| {
                        anyhow::anyhow!("Could not convert workdir_path to UTF-8")
                    })?,
                },
                job_handle_configuration,
            )
            .await
        })
    }

    fn write_input<'a>(
        &'a self,
        workdir_path: &'a Path,
    ) -> Pin<Box<dyn Future<Output = std::io::Result<PathBuf>> + 'a>> {
        match (&self.args.smiles, &self.args.input_mmcif_base64) {
            (Some(input_content), None) => {
                let smiles_file_path = workdir_path.join("acedrg_smiles_input");
                Box::pin(async move {
                    dump_string_to_file(&smiles_file_path, input_content)
                        .await
                        .map(|_nothing| smiles_file_path)
                })
            }
            (None, Some(input_mmcif_base64)) => {
                let mmcif_file_path = workdir_path.join("acedrg_mmcif_input.cif");
                Box::pin(async move {
                    decode_base64_to_file(&mmcif_file_path, input_mmcif_base64)
                        .await
                        .map(|_nothing| mmcif_file_path)
                })
            }
            _ => Box::pin(async {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid input: either SMILES or mmCIF must be provided, but not both.",
                ))
            }),
        }
    }
}
