use std::collections::HashSet;

use crate::error::TaskError;
use crate::{indent_output, output, Result};

pub(crate) mod apt;
pub(crate) mod arch;
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Version(pub String);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PackageStatus {
    pub package: String,
    pub status: InstallStatus,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum InstallStatus {
    Installed(Version),
    Requested,
    NotInstalled,
}

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub name: String,
    pub version: Option<Version>,
}

pub trait PackageManager {
    fn call_update_repos(&self) -> Result<(), TaskError>;
    fn package_status(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError>;
    fn call_install(&self, packages: &[InstallRequest]) -> Result<(), TaskError>;
    fn call_remove(&self, packages: &[&str]) -> Result<(), TaskError>;

    fn install_packages(
        &self,
        packages: &[InstallRequest],
    ) -> Result<Vec<PackageStatus>, TaskError> {
        output!("install packages:");
        let mut requests: Vec<InstallRequest> = Vec::new();
        let package_names: Vec<&str> = packages.iter().map(|r| r.name.as_str()).collect();
        let statuses = self.package_status(&package_names)?;
        for p in statuses {
            let package = packages
                .iter()
                .find(|install| install.name == p.package)
                .ok_or_else(|| {
                    TaskError::Action(format!(
                        "Mismatched requests: could not find request for package {}",
                        p.package
                    ))
                })?;
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

        let statuses = self.package_status(&package_names)?;
        for s in statuses.iter() {
            if let InstallStatus::NotInstalled = s.status {
                return Err(TaskError::Action(format!(
                    "Failed to install {}",
                    s.package
                )));
            }
        }
        Ok(statuses)
    }

    fn remove_packages(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        self.call_remove(packages)?;
        let status = self.package_status(packages)?;
        for s in status.iter() {
            if let InstallStatus::Installed(_s) = &s.status {
                return Err(TaskError::Action(format!("Failed to remove {}", s.package)));
            }
        }
        Ok(status)
    }

    fn ensure(
        &self,
        packages: &[InstallRequest],
        update: bool,
    ) -> Result<(bool, Vec<PackageStatus>), TaskError> {
        let package_names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        let statuses = self.package_status(&package_names)?;
        let missing: HashSet<&str> = statuses
            .iter()
            .filter_map(|p| match p.status {
                InstallStatus::Installed(_) => None,
                _ => Some(p.package.as_str()),
            })
            .collect();
        if missing.is_empty() {
            return Ok((false, statuses));
        }
        if update {
            self.call_update_repos()?;
        }
        let missing_requests: Vec<InstallRequest> = packages
            .iter()
            .filter(|&p| missing.contains(&p.name.as_str()))
            .cloned()
            .collect();

        Ok((true, self.install_packages(&missing_requests)?))
    }
}
