use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fmt::Debug,
    time::Instant,
};

use console::style;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    actions::util::{exec_streaming_process, ProcessOutput},
    debug_output,
    error::TaskError,
    indent_output,
    modules::packaging::{InstallStatus, Version},
};

use super::{InstallRequest, PackageManager, PackageStatus};

pub struct ArchManager {
    manager: String,
}
lazy_static! {
    static ref MISSING_PACKAGE_REGEX: Regex =
        Regex::new("error: package '(.+)' was not found").unwrap();
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
        Ok(status)
    } else {
        Err(TaskError::Action(format!(
            "Failed to parse package status: {}",
            line
        )))
    }
}

impl PackageManager for ArchManager {
    fn call_update_repos(&self) -> crate::Result<(), TaskError> {
        self.must_run_pkg_cmd(&["-Sy"], true)?;
        Ok(())
    }

    fn package_status(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        let started = Instant::now();
        let mut args = vec!["-Qn"];
        args.extend_from_slice(packages);
        let mut statuses = HashSet::new();
        let output = self.run_pkg_cmd(&args, false)?;
        for line in output.stdout.lines() {
            // found packages will show up in stdout
            statuses.insert(parse_package_status(line)?);
        }

        for line in output.stderr.lines() {
            // missing packages will end up here
            if let Some(captures) = MISSING_PACKAGE_REGEX.captures(line) {
                let pkg_name = &captures[1];
                statuses.insert(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::NotInstalled,
                });
            } else {
                indent_output!(
                    1,
                    "{}: Unexpected output: {}",
                    style("WARNING").yellow(),
                    line
                );
            }
        }
        args = vec!["-Qm"];
        args.extend_from_slice(packages);
        let output = self.run_pkg_cmd(&args, false)?;
        for line in output.stdout.lines() {
            // found packages will show up in stdout
            statuses.insert(parse_package_status(line)?);
        }

        for line in output.stderr.lines() {
            // missing packages will end up here
            if let Some(captures) = MISSING_PACKAGE_REGEX.captures(line) {
                let pkg_name = &captures[1];
                statuses.insert(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::NotInstalled,
                });
            } else {
                indent_output!(
                    1,
                    "{}: Unexpected output: {}",
                    style("WARNING").yellow(),
                    line
                );
            }
        }
        //sanity check that number of requested packages matches returned packages
        //TODO: this would probably happen if you repeated a package in the request? should that be caught somewhere?
        if packages.len() != statuses.len() {
            return Err(TaskError::Action(format!(
                "Requested status on {} packages, got {} responses",
                packages.len(),
                statuses.len()
            )));
        }
        debug_output!(
            "Got status for {} packages in {}ms",
            packages.len(),
            started.elapsed().as_millis()
        );
        Ok(statuses.into_iter().collect())
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
