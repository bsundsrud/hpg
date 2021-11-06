use std::process::{Command, Output, Stdio};

use crate::{
    actions::{
        packages::{InstallStatus, PackageStatus, Version},
        process::exit_status,
    },
    error::TaskError,
    WRITER,
};

use super::PackageManager;

pub struct PacmanManager {}

impl PacmanManager {
    pub fn new() -> Self {
        Self {}
    }

    fn call_pacman(&self, args: &[&str]) -> Result<Output, TaskError> {
        let output = Command::new("pacman")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        Ok(output)
    }
}

impl PackageManager for PacmanManager {
    fn call_update_repos(&self) -> Result<(), TaskError> {
        WRITER.write("update repos:");
        let _g = WRITER.enter("update_repo");
        let output = self.call_pacman(&["-Syu", "--noconfirm"])?;
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

    fn package_status(&self, name: &str) -> Result<super::PackageStatus, TaskError> {
        let output = self.call_pacman(&["-Q", name])?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let (package, version) = stdout
                .trim()
                .rsplit_once(" ")
                .ok_or_else(|| TaskError::ActionError(format!("Couldn't get package status")))?;
            WRITER.write(format!("package status: {} {} installed", package, version));
            Ok(PackageStatus {
                package: package.to_string(),
                status: InstallStatus::Installed(Version(version.to_string())),
            })
        } else {
            WRITER.write(format!("package status: {} not installed", name));
            Ok(PackageStatus {
                package: name.to_string(),
                status: InstallStatus::NotInstalled,
            })
        }
    }

    fn call_install(&self, packages: &[super::InstallRequest]) -> crate::Result<(), TaskError> {
        let mut requests: Vec<String> = Vec::new();
        for package in packages {
            if let Some(Version(requested_v)) = &package.version {
                requests.push(format!("{}={}", package.name, requested_v));
            } else {
                requests.push(package.name.clone());
            }
        }
        let mut args = vec!["-S", "--noconfirm", "--noprogressbar"];
        args.extend(requests.iter().map(|s| s.as_str()));
        let output = self.call_pacman(&args)?;

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
        let output =
            self.call_pacman(&["-R", "--noconfirm", "--noprogressbar", &packages.join(" ")])?;
        WRITER.write("remove packages:");
        let _g = WRITER.enter("package_remove");
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
                "Failed removing packages: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}
