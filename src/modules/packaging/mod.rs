use crate::error::TaskError;
use crate::{indent_output, output, Result};

pub(crate) mod apt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(pub String);

#[derive(Debug, Clone)]
pub struct PackageStatus {
    pub package: String,
    pub status: InstallStatus,
}

#[derive(Debug, Clone)]
pub enum InstallStatus {
    Installed(Version),
    Requested,
    NotFound,
    NotInstalled,
}

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub name: String,
    pub version: Option<Version>,
}

pub trait PackageManager {
    fn call_update_repos(&self) -> Result<(), TaskError>;
    fn package_status(&self, name: &str) -> Result<PackageStatus, TaskError>;
    fn call_install(&self, packages: &[InstallRequest]) -> Result<(), TaskError>;
    fn call_remove(&self, packages: &[&str]) -> Result<(), TaskError>;

    fn install_packages(
        &self,
        packages: &[InstallRequest],
    ) -> Result<Vec<PackageStatus>, TaskError> {
        output!("install packages:");
        let mut requests: Vec<InstallRequest> = Vec::new();
        for package in packages {
            let p = self.package_status(&package.name)?;
            if let InstallStatus::Installed(Version(installed_v)) = p.status {
                if let Some(Version(requested_v)) = &package.version {
                    if *requested_v != installed_v {
                        requests.push(package.clone());
                    } else {
                        indent_output!(
                            1,
                            "{} {}: already installed, skipping...",
                            package.name,
                            installed_v
                        );
                    }
                } else {
                    indent_output!(
                        1,
                        "{} {}: already installed, skipping...",
                        package.name,
                        installed_v
                    );
                }
            } else {
                requests.push(package.clone());
            }
        }
        if requests.is_empty() {
            output!("No packages to install.");
            return Ok(Vec::new());
        }
        self.call_install(&requests)?;

        let status = packages
            .iter()
            .map(|p| self.package_status(&p.name))
            .collect::<Result<Vec<PackageStatus>, _>>()?;
        for s in status.iter() {
            match s.status {
                InstallStatus::NotFound | InstallStatus::NotInstalled => {
                    return Err(TaskError::Action(format!(
                        "Failed to install {}",
                        s.package
                    )))
                }
                _ => {}
            }
        }
        Ok(status)
    }

    fn remove_packages(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        self.call_remove(packages)?;
        let status = packages
            .iter()
            .map(|p| self.package_status(p))
            .collect::<Result<Vec<PackageStatus>, _>>()?;
        for s in status.iter() {
            if let InstallStatus::Installed(_s) = &s.status {
                return Err(TaskError::Action(format!("Failed to remove {}", s.package)));
            }
        }
        Ok(status)
    }
}
