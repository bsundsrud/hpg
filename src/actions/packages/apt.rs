use std::process::{Command, Stdio};

use crate::error::TaskError;

use super::PackageManager;

pub struct AptManager {}

impl AptManager {
    pub fn new() -> Self {
        Self {}
    }

    fn call_aptget(&self, args: &[&str]) -> Result<String, TaskError> {
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
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn call_aptcache(&self, args: &[&str]) -> Result<String, TaskError> {
        let output = Command::new("apt-cache")
            .args(args)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            return Err(TaskError::ActionError(format!(
                "Apt-cache call failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl PackageManager for AptManager {
    fn call_update_repos(&self) -> std::result::Result<(), TaskError> {
        let output = self.call_aptget(&["update"])?;
        Ok(())
    }

    fn package_status(&self, name: &str) -> Result<super::PackageStatus, TaskError> {
        todo!()
    }

    fn call_install(&self, packages: &[super::InstallRequest]) -> crate::Result<(), TaskError> {
        todo!()
    }

    fn call_remove(&self, packages: &[&str]) -> crate::Result<(), TaskError> {
        todo!()
    }
}