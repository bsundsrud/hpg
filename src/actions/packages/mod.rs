use rlua::{Lua, Table};

use crate::actions::util::{action_error, task_error};
use crate::error::TaskError;
use crate::Result;
use crate::WRITER;
pub mod apt;
pub mod pacman;

#[derive(Debug, Clone)]
pub struct Version(String);

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
    name: String,
    version: Option<Version>,
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
        WRITER.write("install packages:");
        let _g = WRITER.enter("package_install");
        let mut requests: Vec<InstallRequest> = Vec::new();
        for package in packages {
            let p = self.package_status(&package.name)?;
            if let InstallStatus::Installed(Version(installed_v)) = p.status {
                if let Some(Version(requested_v)) = &package.version {
                    if *requested_v != installed_v {
                        requests.push(package.clone());
                    } else {
                        WRITER.write(format!(
                            "{} {}: already installed, skipping...",
                            package.name, installed_v
                        ));
                    }
                } else {
                    WRITER.write(format!(
                        "{} {}: already installed, skipping...",
                        package.name, installed_v
                    ));
                }
            } else {
                requests.push(package.clone());
            }
        }
        if requests.is_empty() {
            WRITER.write("No packages to install.");
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

    fn remove_packages(&self, packages: &[&str]) -> Result<Vec<PackageStatus>, TaskError> {
        let packages: Vec<&str> = packages.into_iter().map(|p| p.as_ref()).collect();
        self.call_remove(&packages)?;
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

fn value_to_install_request(val: &rlua::Value) -> Result<InstallRequest, rlua::Error> {
    match val {
        rlua::Value::String(s) => Ok(InstallRequest {
            name: s.to_str().unwrap().to_string(),
            version: None,
        }),
        rlua::Value::Table(t) => {
            let name = t.get::<_, String>("name")?;
            let version = t.get::<_, Option<String>>("version")?.map(|v| Version(v));

            Ok(InstallRequest { name, version })
        }
        _ => Err(action_error(
            "Invalid datatype for 'install', must be String or Table",
        )),
    }
}

pub fn package(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (ty, options): (String, Table)| {
            let retval = ctx.create_table()?;
            let update = options.get::<_, Option<bool>>("update")?.unwrap_or(false);
            let install = options
                .get::<_, Option<Vec<rlua::Value>>>("install")?
                .map(|v| {
                    v.iter()
                        .map(value_to_install_request)
                        .collect::<Result<Vec<InstallRequest>, rlua::Error>>()
                });
            let remove = options.get::<_, Option<Vec<String>>>("remove")?;
            let pkg_mgr: Box<dyn PackageManager> = match ty.as_str() {
                "apt" => Box::new(apt::AptManager::new()),
                "pacman" => Box::new(pacman::PacmanManager::new()),
                _ => return Err(action_error("Invalid package manager type")),
            };

            if update {
                pkg_mgr.call_update_repos().map_err(task_error)?;
                retval.set("updated", true)?;
            }
            if let Some(pkgs) = install {
                let p = pkgs?;
                let installed = pkg_mgr.install_packages(&p).map_err(task_error)?;
                let mut installed_lua = Vec::new();
                for p in installed {
                    let tbl = ctx.create_table()?;
                    tbl.set("name", p.package)?;
                    match p.status {
                        InstallStatus::Installed(Version(v)) => {
                            tbl.set("status", "installed")?;
                            tbl.set("version", v)?;
                        }
                        InstallStatus::NotFound => {
                            tbl.set("status", "notfound")?;
                        }
                        InstallStatus::NotInstalled => {
                            tbl.set("status", "notinstalled")?;
                        }
                        InstallStatus::Requested => {
                            tbl.set("status", "requested")?;
                        }
                    }
                    installed_lua.push(tbl);
                }
                retval.set("installed", installed_lua)?;
            }
            if let Some(remove) = remove {
                let r: Vec<&str> = remove.iter().map(|r| r.as_ref()).collect();
                let removed: Vec<String> = pkg_mgr
                    .remove_packages(&r)
                    .map_err(task_error)?
                    .into_iter()
                    .map(|p| p.package)
                    .collect();
                retval.set("removed", removed)?;
            }

            Ok(retval)
        })?;

        lua_ctx.globals().set("packaging", f)?;
        Ok(())
    })?;
    Ok(())
}
