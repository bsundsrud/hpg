use std::{collections::HashMap, ffi::OsStr, fmt::Debug};

use crate::{
    actions::util::{exec_streaming_process, ProcessOutput},
    error::TaskError,
    modules::packaging::{InstallStatus, Version},
};

use super::{InstallRequest, PackageManager, PackageStatus};

pub struct ArchManager {
    manager: String,
}

impl ArchManager {
    pub fn new<S: Into<String>>(manager: S) -> ArchManager {
        Self {
            manager: manager.into(),
        }
    }

    fn run_pkg_cmd<T: AsRef<OsStr> + Debug>(
        &self,
        args: &[T],
        echo: bool,
    ) -> Result<ProcessOutput, TaskError> {
        let output = exec_streaming_process(
            &self.manager,
            args,
            true,
            HashMap::new(),
            None::<&str>,
            true,
            true,
            echo,
        )?;
        if output.status != 0 {
            return Err(TaskError::Action(format!(
                "Failed running {} {:?}:\n  stdout: {}\n  stderr: {}",
                &self.manager, args, output.stdout, output.stderr
            )));
        }
        Ok(output)
    }
}
fn parse_package_status(line: &str) -> Result<PackageStatus, TaskError> {
    if let Some((package, version)) = line.split_once(' ') {
        let status = PackageStatus {
            package: package.to_string(),
            status: InstallStatus::Installed(Version(version.to_string())),
        };
        return Ok(status);
    } else {
        return Err(TaskError::Action(format!(
            "Failed to parse package status: {}",
            line
        )));
    }
}

impl PackageManager for ArchManager {
    fn call_update_repos(&self) -> crate::Result<(), TaskError> {
        self.run_pkg_cmd(&["-Sy"], true)?;
        Ok(())
    }

    fn package_status(&self, name: &str) -> Result<PackageStatus, TaskError> {
        let native_output = self.run_pkg_cmd(&["-Qn", name], false)?;
        let lines: Vec<String> = native_output
            .stdout
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.len() == 0 {
            //skip, may be from AUR
        } else if lines.len() == 1 {
            let status = parse_package_status(&lines[0])?;
            return Ok(status);
        } else {
            //didn't expect multiple lines here
            return Err(TaskError::Action(format!(
                "received multiple package statuses for {}",
                name
            )));
        }

        let foreign_output = self.run_pkg_cmd(&["-Qm", name], false)?;
        let lines: Vec<String> = foreign_output
            .stdout
            .lines()
            .map(|s| s.to_string())
            .collect();
        if lines.len() == 0 {
            return Ok(PackageStatus {
                package: name.into(),
                status: InstallStatus::NotInstalled,
            });
        } else if lines.len() == 1 {
            let status = parse_package_status(&lines[0])?;
            return Ok(status);
        } else {
            //didn't expect multiple lines here
            return Err(TaskError::Action(format!(
                "received multiple package statuses for {}",
                name
            )));
        }
    }

    fn call_install(&self, packages: &[InstallRequest]) -> Result<(), TaskError> {
        let mut args: Vec<String> = vec![
            "-Syu".to_string(),
            "--needed".to_string(),
            "--noconfirm".to_string(),
        ];
        for p in packages {
            if let Some(v) = &p.version {
                args.push(format!("{}={}", p.name, v.0));
            }
        }
        self.run_pkg_cmd(&args, true)?;
        Ok(())
    }

    fn call_remove(&self, packages: &[&str]) -> Result<(), TaskError> {
        let mut args = vec!["-R", "--noconfirm"];
        args.extend_from_slice(packages);
        self.run_pkg_cmd(&args, true)?;
        Ok(())
    }
}
