use rlua::{Lua, Table};

use crate::actions::util::{action_error, task_error};
use crate::error::TaskError;
use crate::Result;
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
                pkg_mgr.update_repos().map_err(task_error)?;
            }
            if let Some(pkgs) = install {
                let p = pkgs?;
                pkg_mgr.install_packages(&p).map_err(task_error)?;
            }
            if let Some(remove) = remove {
                let r: Vec<&str> = remove.iter().map(|r| r.as_ref()).collect();
                pkg_mgr.remove_packages(&r).map_err(task_error)?;
            }
            Ok(retval)
        })?;

        lua_ctx.globals().set("packaging", f)?;
        Ok(())
    })?;
    Ok(())
}
