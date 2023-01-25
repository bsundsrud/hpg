use crate::{
    error::{self, TaskError},
    Result, WRITER,
};
use mlua::{Lua, Table};

use super::packaging::{
    apt::AptManager, InstallRequest, InstallStatus, PackageManager, PackageStatus, Version,
};

pub fn pkg(lua: &Lua) -> Result<(), TaskError> {
    let t = lua.create_table()?;

    t.set("apt", apt(&lua)?)?;
    lua.globals().set("pkg", t)?;

    Ok(())
}

fn apt(ctx: &Lua) -> Result<Table, mlua::Error> {
    let tbl = ctx.create_table()?;
    tbl.set("_updated", false)?;
    let update = ctx.create_function(|ctx, force: Option<bool>| {
        let do_update = if let Some(true) = force {
            true
        } else {
            let already_updated = ctx
                .globals()
                .get::<_, Table>("pkg")?
                .get::<_, Table>("apt")?
                .get::<_, bool>("_updated")?;
            !already_updated
        };
        if !do_update {
            WRITER.write("update repos: skip");
            return Ok(do_update);
        }
        let apt = AptManager::new();
        apt.call_update_repos().map_err(error::task_error)?;
        ctx.globals()
            .get::<_, Table>("pkg")?
            .get::<_, Table>("apt")?
            .set("_updated", true)?;
        Ok(do_update)
    })?;
    tbl.set("update", update)?;

    let status = ctx.create_function(|ctx, name: String| {
        let apt = AptManager::new();
        let status = apt.package_status(&name).map_err(error::task_error)?;
        Ok(package_status_to_lua(ctx, &status)?)
    })?;
    tbl.set("status", status)?;

    let install = ctx.create_function(|ctx, reqs: Vec<mlua::Value>| {
        let packages = reqs
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, mlua::Error>>()?;
        let apt = AptManager::new();
        let installed = apt.install_packages(&packages).map_err(error::task_error)?;
        let res = installed
            .into_iter()
            .map(|i| package_status_to_lua(ctx.clone(), &i))
            .collect::<Result<Vec<Table<'_>>, mlua::Error>>()?;
        Ok(res)
    })?;
    tbl.set("install", install)?;

    let remove = ctx.create_function(|ctx, packages: Vec<String>| {
        let apt = AptManager::new();
        let r: Vec<&str> = packages.iter().map(|r| r.as_ref()).collect();
        let packages = apt
            .remove_packages(&r)
            .map_err(error::task_error)?
            .into_iter()
            .map(|p| package_status_to_lua(ctx.clone(), &p))
            .collect::<Result<Vec<Table<'_>>, mlua::Error>>()?;
        Ok(packages)
    })?;
    tbl.set("remove", remove)?;

    let ensure = ctx.create_function(|ctx, packages: Vec<mlua::Value>| {
        let apt = AptManager::new();
        let mut found_missing = false;
        let packages = packages
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, mlua::Error>>()?;

        for p in packages.iter() {
            let status = apt.package_status(&p.name).map_err(error::task_error)?;
            match status.status {
                // If a requested package is missing/not installed, try to install the whole batch
                InstallStatus::NotFound | InstallStatus::NotInstalled => {
                    found_missing = true;
                    break;
                }
                // If a requested package is installed but at the wrong version, try to install the whole batch
                InstallStatus::Installed(ref v) => {
                    if let Some(ref requested_version) = p.version {
                        if requested_version != v {
                            found_missing = true;
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        let res_tbl = ctx.create_table()?;
        if found_missing {
            WRITER.write("Ensure: Packages differ from request.");
            // Use warm apt cache if available, otherwise refresh
            let already_updated = ctx
                .globals()
                .get::<_, Table>("pkg")?
                .get::<_, Table>("apt")?
                .get::<_, bool>("_updated")?;
            if !already_updated {
                apt.call_update_repos().map_err(error::task_error)?;
            }

            let installed = apt.install_packages(&packages).map_err(error::task_error)?;
            let results = installed
                .into_iter()
                .map(|i| package_status_to_lua(ctx.clone(), &i))
                .collect::<Result<Vec<Table<'_>>, mlua::Error>>()?;
            res_tbl.set("updated", true)?;
            res_tbl.set("packages", results)?;
        } else {
            WRITER.write("Ensure: Packages all up-to-date.");
            res_tbl.set("updated", false)?;
            let blank = ctx.create_table()?;
            res_tbl.set("packages", blank)?;
        }
        Ok(res_tbl)
    })?;
    tbl.set("ensure", ensure)?;

    Ok(tbl)
}

fn package_status_to_lua<'lua>(
    ctx: &'lua Lua,
    p: &PackageStatus,
) -> Result<Table<'lua>, mlua::Error> {
    let tbl = ctx.create_table()?;
    tbl.set("name", p.package.as_str())?;
    match &p.status {
        InstallStatus::Installed(Version(v)) => {
            tbl.set("status", "installed")?;
            tbl.set("version", v.as_str())?;
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
    Ok(tbl)
}

fn value_to_install_request(val: &mlua::Value) -> Result<InstallRequest, mlua::Error> {
    match val {
        mlua::Value::String(s) => Ok(InstallRequest {
            name: s.to_str().unwrap().to_string(),
            version: None,
        }),
        mlua::Value::Table(t) => {
            let name = t.get::<_, String>("name")?;
            let version = t.get::<_, Option<String>>("version")?.map(|v| Version(v));

            Ok(InstallRequest { name, version })
        }
        _ => Err(error::action_error(
            "Invalid datatype for 'install', must be String or Table",
        )),
    }
}
