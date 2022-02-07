use crate::{actions::util, error::TaskError, Result, WRITER};
use rlua::{Context, Lua, Table};

use super::packaging::{
    apt::AptManager, InstallRequest, InstallStatus, PackageManager, PackageStatus, Version,
};

pub fn pkg(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let t = lua_ctx.create_table()?;

        t.set("apt", apt(lua_ctx.clone())?)?;
        lua_ctx.globals().set("pkg", t)?;
        Ok(())
    })?;

    Ok(())
}

fn apt<'lua>(ctx: Context<'lua>) -> Result<Table<'lua>, rlua::Error> {
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
        apt.call_update_repos().map_err(util::task_error)?;
        ctx.globals()
            .get::<_, Table>("pkg")?
            .get::<_, Table>("apt")?
            .set("_updated", true)?;
        Ok(do_update)
    })?;
    tbl.set("update", update)?;

    let status = ctx.create_function(|ctx, name: String| {
        let apt = AptManager::new();
        let status = apt.package_status(&name).map_err(util::task_error)?;
        Ok(package_status_to_lua(ctx, &status)?)
    })?;
    tbl.set("status", status)?;

    let install = ctx.create_function(|ctx, reqs: Vec<rlua::Value>| {
        let packages = reqs
            .iter()
            .map(value_to_install_request)
            .collect::<Result<Vec<InstallRequest>, rlua::Error>>()?;
        let apt = AptManager::new();
        let installed = apt.install_packages(&packages).map_err(util::task_error)?;
        let res = installed
            .into_iter()
            .map(|i| package_status_to_lua(ctx.clone(), &i))
            .collect::<Result<Vec<Table<'_>>, rlua::Error>>()?;
        Ok(res)
    })?;
    tbl.set("install", install)?;

    let remove = ctx.create_function(|ctx, packages: Vec<String>| {
        let apt = AptManager::new();
        let r: Vec<&str> = packages.iter().map(|r| r.as_ref()).collect();
        let packages = apt
            .remove_packages(&r)
            .map_err(util::task_error)?
            .into_iter()
            .map(|p| package_status_to_lua(ctx.clone(), &p))
            .collect::<Result<Vec<Table<'_>>, rlua::Error>>()?;
        Ok(packages)
    })?;
    tbl.set("remove", remove)?;

    Ok(tbl)
}

fn package_status_to_lua<'lua>(
    ctx: Context<'lua>,
    p: &PackageStatus,
) -> Result<Table<'lua>, rlua::Error> {
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
        _ => Err(util::action_error(
            "Invalid datatype for 'install', must be String or Table",
        )),
    }
}
