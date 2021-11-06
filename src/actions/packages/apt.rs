use std::process::{Command, Output, Stdio};

use crate::{
    actions::{packages::Version, process::exit_status},
    error::TaskError,
    WRITER,
};

use super::{InstallStatus, PackageManager, PackageStatus};

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
            .arg("${status}||${version}")
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
                    status: InstallStatus::NotInstalled,
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
        let version = parts
            .next()
            .ok_or_else(|| TaskError::ActionError(format!("Bad dpkg-query version: {}", stdout)))?;
        let requested_install = status.starts_with("install");
        let is_installed = status.ends_with("installed");
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
        WRITER.write("update repos:");
        let _g = WRITER.enter("update_repo");
        let output = self.call_aptget(&["update"])?;
        if output.stdout.len() > 0 {
            WRITER.write("stdout:");
            let _g = WRITER.enter("stdout");
            WRITER.write(String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            WRITER.write("stderr:");
            let _g = WRITER.enter("stderr");
            WRITER.write(String::from_utf8_lossy(&output.stderr));
        }
        WRITER.write(format!("exit code: {}", exit_status(&output.status)));
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
        let output = self.call_aptget(&args)?;
        if output.stdout.len() > 0 {
            WRITER.write("stdout:");
            let _g = WRITER.enter("stdout");
            WRITER.write(String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            WRITER.write("stderr:");
            let _g = WRITER.enter("stderr");
            WRITER.write(String::from_utf8_lossy(&output.stderr));
        }
        WRITER.write(format!("exit code: {}", exit_status(&output.status)));

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
        let output = self.call_aptget(&args)?;
        if output.stdout.len() > 0 {
            WRITER.write("stdout:");
            let _g = WRITER.enter("stdout");
            WRITER.write(String::from_utf8_lossy(&output.stdout));
        }
        if output.stderr.len() > 0 {
            WRITER.write("stderr:");
            let _g = WRITER.enter("stderr");
            WRITER.write(String::from_utf8_lossy(&output.stderr));
        }
        WRITER.write(format!("exit code: {}", exit_status(&output.status)));

        if !output.status.success() {
            return Err(TaskError::ActionError(format!(
                "Failed installing packages: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}
