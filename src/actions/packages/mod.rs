use crate::error::TaskError;

pub mod apt;
pub mod pacman;

pub struct Version(String);

pub struct PackageStatus {
    pub package: String,
    pub status: InstallStatus,
}

pub enum InstallStatus {
    Installed(Version),
    NotFound,
    NotInstalled,
}

pub struct InstallRequest {
    name: String,
    version: Option<Version>,
}

pub trait PackageManager {
    fn update_repos(&self) -> Result<String, TaskError>;
    fn package_status(&self, name: &str) -> Result<PackageStatus, TaskError>;
    fn install_packages(
        &self,
        packages: &[InstallRequest],
    ) -> Result<Vec<PackageStatus>, TaskError>;
    fn remove_packages(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError>;
}
