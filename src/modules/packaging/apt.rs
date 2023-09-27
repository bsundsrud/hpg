use std::process::{Command, Output, Stdio};

use crate::{actions::util::exit_status, error::TaskError, indent_output, output};

use super::{InstallStatus, PackageManager, PackageStatus, Version};

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
            return Err(TaskError::ActionError(format!(
                "Apt-get call failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(output)
    }

    fn call_dpkg_query(&self, pkg: &str) -> Result<PackageStatus, TaskError> {
        let output = Command::new("dpkg-query")
            .arg("-f")
            .arg("${db:Status-Status}||${db:Status-Want}||${Version}")
            .arg("-W")
            .arg(pkg)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no packages found") {
                return Ok(PackageStatus {
                    package: pkg.into(),
                    status: InstallStatus::NotFound,
                });
            }
            return Err(TaskError::ActionError(format!(
                "Dpkg-query call failed: {}",
                stderr
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut parts = stdout.split("||");
        let status = parts
            .next()
            .ok_or_else(|| TaskError::ActionError(format!("Bad dpkg-query status: {}", stdout)))?;
        let desired = parts.next().ok_or_else(|| {
            TaskError::ActionError(format!("Bad dpkg-query status-want: {}", stdout))
        })?;
        let version = parts
            .next()
            .ok_or_else(|| TaskError::ActionError(format!("Bad dpkg-query version: {}", stdout)))?;
        let requested_install = desired.starts_with("install");
        let is_installed = status.starts_with("installed");
        if requested_install && is_installed {
            Ok(PackageStatus {
                package: pkg.into(),
                status: InstallStatus::Installed(Version(version.into())),
            })
        } else if requested_install && !is_installed {
            Ok(PackageStatus {
                package: pkg.into(),
                status: InstallStatus::Requested,
            })
        } else {
            Ok(PackageStatus {
                package: pkg.into(),
                status: InstallStatus::NotInstalled,
            })
        }
    }
}

impl PackageManager for AptManager {
    fn call_update_repos(&self) -> Result<(), TaskError> {
        output!("update repos:");
        let output = self.call_aptget(&["update"])?;
        if output.stdout.len() > 0 {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));
        if output.status.success() {
            Ok(())
        } else {
            Err(TaskError::ActionError(format!(
                "Failed updating repos: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn package_status(&self, name: &str) -> Result<PackageStatus, TaskError> {
        self.call_dpkg_query(name)
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
        if output.stdout.len() > 0 {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));

        if !output.status.success() {
            return Err(TaskError::ActionError(format!(
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
        if output.stdout.len() > 0 {
            indent_output!(1, "stdout:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            indent_output!(1, "stderr:");
            indent_output!(2, "{}", String::from_utf8_lossy(&output.stderr));
        }
        indent_output!(1, "exit code: {}", exit_status(&output.status));

        if !output.status.success() {
            return Err(TaskError::ActionError(format!(
                "Failed installing packages: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}
