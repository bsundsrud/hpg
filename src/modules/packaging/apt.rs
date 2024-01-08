use std::{
    collections::HashMap,
    process::{Command, Output, Stdio},
};

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    actions::util::{exec_streaming_process, exit_status},
    error::TaskError,
    indent_output, output,
};

use super::{InstallStatus, PackageManager, PackageStatus, Version};

lazy_static! {
    static ref DPKG_REGEX: Regex =
        regex::Regex::new("dpkg-query: no packages found matching (.*)").unwrap();
}
pub struct AptManager {}

impl AptManager {
    pub fn new() -> Self {
        Self {}
    }

    fn call_aptget(&self, args: &[&str]) -> Result<Output, TaskError> {
        let output = Command::new("apt-get")
            .args(args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            return Err(TaskError::Action(format!(
                "Apt-get call failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(output)
    }

    fn call_dpkg_query(&self, pkgs: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        let mut args = vec![
            "-f",
            "${Package}||${db:Status-Status}||${db:Status-Want}||${Version}\n",
            "-W",
        ];
        args.extend_from_slice(pkgs);
        let output = exec_streaming_process(
            "dpkg-query",
            args,
            true,
            HashMap::new(),
            None::<&str>,
            true,
            true,
            false,
        )?;
        let mut statuses = Vec::new();
        for line in output.stdout.lines() {
            //stdout will have found packages
            let mut parts = line.split("||");
            let pkg_name = parts
                .next()
                .ok_or_else(|| TaskError::Action(format!("Bad dpkg-query status: {}", line)))?;
            let status = parts
                .next()
                .ok_or_else(|| TaskError::Action(format!("Bad dpkg-query status: {}", line)))?;
            let desired = parts.next().ok_or_else(|| {
                TaskError::Action(format!("Bad dpkg-query status-want: {}", line))
            })?;
            let version = parts
                .next()
                .ok_or_else(|| TaskError::Action(format!("Bad dpkg-query version: {}", line)))?;
            let requested_install = desired.starts_with("install");
            let is_installed = status.starts_with("installed");
            if requested_install && is_installed {
                statuses.push(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::Installed(Version(version.into())),
                });
            } else if requested_install && !is_installed {
                statuses.push(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::Requested,
                })
            } else {
                statuses.push(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::NotInstalled,
                })
            }
        }

        for line in output.stderr.lines() {
            // missing packages will be in stderr
            if let Some(captures) = DPKG_REGEX.captures(line) {
                let pkg_name = &captures[1];
                statuses.push(PackageStatus {
                    package: pkg_name.into(),
                    status: InstallStatus::NotInstalled,
                })
            } else {
                return Err(TaskError::Action(format!(
                    "Unrecognized dpkg-query output: {}",
                    line
                )));
            }
        }
        // sanity check: make sure we have statuses for all requested packages
        if pkgs.len() != statuses.len() {
            return Err(TaskError::Action(format!(
                "Requested statuses for {} packages, only received {}",
                pkgs.len(),
                statuses.len()
            )));
        }
        Ok(statuses)
    }
}

impl PackageManager for AptManager {
    fn call_update_repos(&self) -> Result<(), TaskError> {
        output!("update repos:");
        let output = self.call_aptget(&["update"])?;
        if !output.stdout.is_empty() {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));
        if output.status.success() {
            Ok(())
        } else {
            Err(TaskError::Action(format!(
                "Failed updating repos: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn package_status(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        self.call_dpkg_query(packages)
    }

    fn call_install(&self, packages: &[super::InstallRequest]) -> crate::Result<(), TaskError> {
        let packages: Vec<String> = packages
            .iter()
            .map(|i| {
                if let Some(Version(v)) = &i.version {
                    format!("{}={}", i.name, v)
                } else {
                    i.name.clone()
                }
            })
            .collect();
        let mut args = vec!["install", "-y"];
        args.extend(packages.iter().map(|s| s.as_str()));
        output!("install:");
        let output = self.call_aptget(&args)?;
        if !output.stdout.is_empty() {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));

        if !output.status.success() {
            return Err(TaskError::Action(format!(
                "Failed installing packages: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    fn call_remove(&self, packages: &[&str]) -> crate::Result<(), TaskError> {
        let mut args = vec!["remove", "-y"];
        args.extend(packages);
        output!("remove:");
        let output = self.call_aptget(&args)?;
        if !output.stdout.is_empty() {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));

        if !output.status.success() {
            return Err(TaskError::Action(format!(
                "Failed installing packages: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}
