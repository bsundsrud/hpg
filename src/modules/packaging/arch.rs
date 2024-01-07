use std::{collections::HashMap, ffi::OsStr, fmt::Debug, time::Instant};

use crate::{
    actions::util::{exec_streaming_process, ProcessOutput},
    debug_output,
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

        Ok(output)
    }

    fn must_run_pkg_cmd<T: AsRef<OsStr> + Debug>(
        &self,
        args: &[T],
        echo: bool,
    ) -> Result<ProcessOutput, TaskError> {
        let output = self.run_pkg_cmd(args, echo)?;
        if output.status != 0 {
            return Err(TaskError::Action(format!(
                "Failed running {} {:?}, exit {}:\n  stdout: {}\n  stderr: {}",
                &self.manager, args, output.status, output.stdout, output.stderr
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
        self.must_run_pkg_cmd(&["-Sy"], true)?;
        Ok(())
    }

    fn package_status(&self, name: &str) -> Result<PackageStatus, TaskError> {
        let started = Instant::now();
        let output = self.run_pkg_cmd(&["-Qn", name], false)?;
        let lines: Vec<String> = output.stdout.lines().map(|s| s.to_string()).collect();
        if output.status == 1 && lines.len() == 0 {
            //skip, may be from AUR
        } else if lines.len() == 1 {
            let status = parse_package_status(&lines[0])?;
            debug_output!(
                "Checked package {} in {}ms",
                name,
                started.elapsed().as_millis()
            );
            return Ok(status);
        } else {
            //didn't expect multiple lines here
            return Err(TaskError::Action(format!(
                "received unexpected for {} -Qn {}:\n  stdout: {}\n  stderr: {}",
                self.manager, name, output.stdout, output.stderr
            )));
        }

        let output = self.run_pkg_cmd(&["-Qm", name], false)?;
        let lines: Vec<String> = output.stdout.lines().map(|s| s.to_string()).collect();
        if output.status == 1 && lines.len() == 0 {
            debug_output!(
                "Checked package {} in {}ms",
                name,
                started.elapsed().as_millis()
            );
            return Ok(PackageStatus {
                package: name.into(),
                status: InstallStatus::NotInstalled,
            });
        } else if lines.len() == 1 {
            let status = parse_package_status(&lines[0])?;
            debug_output!(
                "Checked package {} in {}ms",
                name,
                started.elapsed().as_millis()
            );
            return Ok(status);
        } else {
            //didn't expect multiple lines here
            return Err(TaskError::Action(format!(
                "received unexpected for {} -Qn {}:\n  stdout: {}\n  stderr: {}",
                self.manager, name, output.stdout, output.stderr
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
            } else {
                args.push(p.name.clone());
            }
        }
        self.must_run_pkg_cmd(&args, true)?;
        Ok(())
    }

    fn call_remove(&self, packages: &[&str]) -> Result<(), TaskError> {
        let mut args = vec!["-R", "--noconfirm"];
        args.extend_from_slice(packages);
        self.must_run_pkg_cmd(&args, true)?;
        Ok(())
    }
}
