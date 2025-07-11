use crate::{
    error::{self, TaskError},
    indent_output, output, Result,
};
use mlua::{Lua, Table};

use super::packaging::{
    apt::AptManager, arch::ArchManager, InstallRequest, InstallStatus, PackageManager,
    PackageStatus, Version,
};

pub fn pkg(lua: &Lua) -> Result<(), TaskError> {
    let t = lua.create_table()?;

    t.set("apt", apt(lua)?)?;
    t.set("arch", arch(lua)?)?;
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
                .get::<Table>("pkg")?
                .get::<Table>("apt")?
                .get::<bool>("_updated")?;
            !already_updated
        };
        if !do_update {
            output!("update repos: skip");
            return Ok(do_update);
        }
        let apt = AptManager::new();
        apt.call_update_repos().map_err(error::task_error)?;
        ctx.globals()
            .get::<Table>("pkg")?
            .get::<Table>("apt")?
            .set("_updated", true)?;
        Ok(do_update)
    })?;
    tbl.set("update", update)?;

    let status = ctx.create_function(|ctx, name: String| {
        let apt = AptManager::new();
        let status = apt.package_status(&[&name]).map_err(error::task_error)?;
        package_status_to_lua(ctx, &status[0])
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
            .map(|i| package_status_to_lua(ctx, &i))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
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
            .map(|p| package_status_to_lua(ctx, &p))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
        Ok(packages)
    })?;
    tbl.set("remove", remove)?;

    let ensure = ctx.create_function(|ctx, packages: Vec<mlua::Value>| {
        let apt = AptManager::new();
        let packages = packages
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, mlua::Error>>()?;
        let already_updated = ctx
            .globals()
            .get::<Table>("pkg")?
            .get::<Table>("apt")?
            .get::<bool>("_updated")?;
        output!(
            "Ensure Packages: {}",
            packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        );
        let (updated, statuses) = apt
            .ensure(&packages, !already_updated)
            .map_err(error::task_error)?;
        let res_tbl = ctx.create_table()?;
        if !updated {
            indent_output!(1, "Ensure: Packages all up-to-date.");
        } else {
            indent_output!(
                1,
                "Ensure: Installed {}",
                statuses
                    .iter()
                    .map(|s| s.package.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }
        let results = statuses
            .into_iter()
            .map(|i| package_status_to_lua(ctx, &i))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
        res_tbl.set("updated", updated)?;
        res_tbl.set("packages", results)?;

        Ok(res_tbl)
    })?;
    tbl.set("ensure", ensure)?;

    Ok(tbl)
}

fn get_arch_manager(ctx: &Lua) -> String {
    ctx.globals()
        .get::<Table>("pkg")
        .unwrap()
        .get::<Table>("arch")
        .unwrap()
        .get::<String>("package_manager")
        .unwrap()
}

fn arch(ctx: &Lua) -> Result<Table, mlua::Error> {
    let tbl = ctx.create_table()?;
    tbl.set("_updated", false)?;
    tbl.set("package_manager", "pacman")?;
    let update = ctx.create_function(|ctx, force: Option<bool>| {
        let do_update = if let Some(true) = force {
            true
        } else {
            let already_updated = ctx
                .globals()
                .get::<Table>("pkg")?
                .get::<Table>("arch")?
                .get::<bool>("_updated")?;
            !already_updated
        };
        if !do_update {
            output!("update repos: skip");
            return Ok(do_update);
        }

        let pacman = ArchManager::new(get_arch_manager(ctx));
        pacman.call_update_repos().map_err(error::task_error)?;
        ctx.globals()
            .get::<Table>("pkg")?
            .get::<Table>("arch")?
            .set("_updated", true)?;
        Ok(do_update)
    })?;
    tbl.set("update", update)?;

    let status = ctx.create_function(|ctx, name: String| {
        let pacman = ArchManager::new(get_arch_manager(ctx));
        let status = pacman.package_status(&[&name]).map_err(error::task_error)?;
        package_status_to_lua(ctx, &status[0])
    })?;
    tbl.set("status", status)?;

    let install = ctx.create_function(|ctx, reqs: Vec<mlua::Value>| {
        let packages = reqs
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, mlua::Error>>()?;
        let pacman = ArchManager::new(get_arch_manager(ctx));
        let installed = pacman
            .install_packages(&packages)
            .map_err(error::task_error)?;
        let res = installed
            .into_iter()
            .map(|i| package_status_to_lua(ctx, &i))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
        Ok(res)
    })?;
    tbl.set("install", install)?;

    let remove = ctx.create_function(|ctx, packages: Vec<String>| {
        let pacman = ArchManager::new(get_arch_manager(ctx));
        let r: Vec<&str> = packages.iter().map(|r| r.as_ref()).collect();
        let packages = pacman
            .remove_packages(&r)
            .map_err(error::task_error)?
            .into_iter()
            .map(|p| package_status_to_lua(ctx, &p))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
        Ok(packages)
    })?;
    tbl.set("remove", remove)?;

    let ensure = ctx.create_function(|ctx, packages: Vec<mlua::Value>| {
        let pacman = ArchManager::new(get_arch_manager(ctx));
        let packages = packages
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, mlua::Error>>()?;
        let already_updated = ctx
            .globals()
            .get::<Table>("pkg")?
            .get::<Table>("arch")?
            .get::<bool>("_updated")?;
        output!(
            "Ensure Packages: {}",
            packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<&str>>()
                .join(", ")
        );
        let (updated, statuses) = pacman
            .ensure(&packages, !already_updated)
            .map_err(error::task_error)?;
        let res_tbl = ctx.create_table()?;
        if !updated {
            indent_output!(1, "Ensure: Packages all up-to-date.");
        } else {
            indent_output!(
                1,
                "Ensure: Installed {}",
                statuses
                    .iter()
                    .map(|s| s.package.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }
        let results = statuses
            .into_iter()
            .map(|i| package_status_to_lua(ctx, &i))
            .collect::<Result<Vec<Table>, mlua::Error>>()?;
        res_tbl.set("updated", updated)?;
        res_tbl.set("packages", results)?;

        Ok(res_tbl)
    })?;
    tbl.set("ensure", ensure)?;

    Ok(tbl)
}

fn package_status_to_lua(ctx: &Lua, p: &PackageStatus) -> Result<Table, mlua::Error> {
    let tbl = ctx.create_table()?;
    tbl.set("name", p.package.as_str())?;
    match &p.status {
        InstallStatus::Installed(Version(v)) => {
            tbl.set("status", "installed")?;
            tbl.set("version", v.as_str())?;
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
            let name = t.get::<String>("name")?;
            let version = t.get::<Option<String>>("version")?.map(Version);

            Ok(InstallRequest { name, version })
        }
        _ => Err(error::action_error(
            "Invalid datatype for 'install', must be String or Table",
        )),
    }
}
