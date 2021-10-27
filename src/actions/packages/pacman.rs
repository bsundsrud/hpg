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
    fn update_repos(&self) -> Result<String, TaskError> {
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
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(TaskError::ActionError(format!(
                "Failed updating repos: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn package_status(&self, name: &str) -> Result<super::PackageStatus, TaskError> {
        let output = self.call_pacman(&["-D", name])?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let (package, version) = stdout
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

    fn install_packages(
        &self,
        packages: &[super::InstallRequest],
    ) -> Result<Vec<super::PackageStatus>, TaskError> {
        let requests: Vec<String> = packages
            .iter()
            .map(|r| {
                if let Some(v) = &r.version {
                    format!("{}={}", r.name, v.0)
                } else {
                    r.name.clone()
                }
            })
            .collect();
        WRITER.write("install packages:");
        let _g = WRITER.enter("package_install");
        let output =
            self.call_pacman(&["-S", "--noconfirm", "--noprogressbar", &requests.join(" ")])?;

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
        let status = packages
            .iter()
            .map(|p| self.package_status(&p.name))
            .collect::<Result<Vec<PackageStatus>, _>>()?;
        for s in status.iter() {
            match s.status {
                InstallStatus::NotFound | InstallStatus::NotInstalled => {
                    return Err(TaskError::ActionError(format!(
                        "Failed to install {}",
                        s.package
                    )))
                }
                _ => {}
            }
        }
        Ok(status)
    }

    fn remove_packages(&self, packages: &[&str]) -> Result<Vec<super::PackageStatus>, TaskError> {
        let output = self.call_pacman(&["-R", &packages.join(" ")])?;
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
        let status = packages
            .iter()
            .map(|p| self.package_status(p))
            .collect::<Result<Vec<PackageStatus>, _>>()?;
        for s in status.iter() {
            match s.status {
                InstallStatus::Installed(_) => {
                    return Err(TaskError::ActionError(format!(
                        "Failed to remove {}",
                        s.package
                    )))
                }
                _ => {}
            }
        }
        Ok(status)
    }
}
