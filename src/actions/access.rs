use mlua::{Error as LuaError, Lua, Table};

use crate::error::{io_error, task_error, TaskError};
use crate::{output, Result};
use std::{io::Error as IoError, process::Command};

use super::util::exit_status;

#[derive(Debug)]
struct UserModDef {
    name: String,
    comment: Option<String>,
    home_dir: Option<String>,
    primary_group: Option<String>,
    groups: Vec<String>,
    user_group: bool,
    create_home: bool,
    system: bool,
    uid: Option<u32>,
    shell: String,
}

impl UserModDef {
    fn from_lua<'a>(name: String, opts: Table<'a>) -> Result<UserModDef, LuaError> {
        let comment = opts.get::<_, Option<String>>("comment")?;
        let home_dir = opts.get::<_, Option<String>>("home_dir")?;
        let primary_group = opts.get::<_, Option<String>>("group")?;
        let groups = opts
            .get::<_, Option<Vec<String>>>("groups")?
            .unwrap_or_else(|| Vec::new());
        let system = opts.get::<_, Option<bool>>("is_system")?.unwrap_or(false);
        let user_group = opts
            .get::<_, Option<bool>>("create_user_group")?
            .unwrap_or(true);
        let create_home = opts.get::<_, Option<bool>>("create_home")?.unwrap_or(false);
        let uid = opts.get::<_, Option<u32>>("uid")?;
        let shell = opts
            .get::<_, Option<String>>("shell")?
            .unwrap_or_else(|| "/usr/bin/nologin".to_string());
        Ok(UserModDef {
            name,
            comment,
            home_dir,
            primary_group,
            groups,
            user_group,
            create_home,
            system,
            uid,
            shell,
        })
    }
}

#[derive(Debug)]
struct GroupModDef {
    name: String,
    system: bool,
    gid: Option<u32>,
}

impl GroupModDef {
    fn from_lua<'a>(name: String, opts: Table<'a>) -> Result<GroupModDef, LuaError> {
        let system = opts.get::<_, Option<bool>>("is_system")?.unwrap_or(false);
        let gid = opts.get::<_, Option<u32>>("gid")?;
        Ok(GroupModDef { name, system, gid })
    }
}

fn create_user(user: UserModDef) -> Result<(), TaskError> {
    let mut cmd = Command::new("useradd");
    cmd.arg("-s").arg(user.shell);
    if let Some(comment) = user.comment {
        cmd.arg("-c").arg(comment);
    }
    if let Some(home_dir) = user.home_dir {
        cmd.arg("-d").arg(home_dir);
    }
    if let Some(g) = user.primary_group {
        cmd.arg("-g").arg(g);
    }
    if !user.groups.is_empty() {
        cmd.arg("-G").arg(user.groups.join(","));
    }

    if user.create_home {
        cmd.arg("-m");
    } else {
        cmd.arg("-M");
    }

    if user.user_group {
        cmd.arg("-U");
    } else {
        cmd.arg("-N");
    }

    if user.system {
        cmd.arg("--system");
    }

    if let Some(uid) = user.uid {
        cmd.arg("-u").arg(uid.to_string());
    }

    cmd.arg(user.name);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(TaskError::ActionError(format!(
            "Bad useradd exit {}: {} {}",
            exit_status(&output.status),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn modify_user(user: UserModDef) -> Result<(), TaskError> {
    let mut cmd = Command::new("usermod");
    cmd.arg("-s").arg(user.shell);
    if let Some(comment) = user.comment {
        cmd.arg("-c").arg(comment);
    }
    if let Some(home_dir) = user.home_dir {
        cmd.arg("-d").arg(home_dir);
    }
    if let Some(g) = user.primary_group {
        cmd.arg("-g").arg(g);
    }
    if !user.groups.is_empty() {
        cmd.arg("-G").arg(user.groups.join(","));
    }

    if user.create_home {
        cmd.arg("-m");
    }

    if let Some(uid) = user.uid {
        cmd.arg("-u").arg(uid.to_string());
    }

    cmd.arg(user.name);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(TaskError::ActionError(format!(
            "Bad usermod exit {}: {} {}",
            exit_status(&output.status),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn user_exists(name: &str) -> Result<bool, IoError> {
    let cmd = Command::new("id").arg(name).output()?;
    Ok(cmd.status.success())
}

fn create_group(group: GroupModDef) -> Result<(), TaskError> {
    let mut cmd = Command::new("groupadd");
    if let Some(gid) = group.gid {
        cmd.arg("-g").arg(gid.to_string());
    }
    if group.system {
        cmd.arg("--system");
    }
    cmd.arg(group.name);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(TaskError::ActionError(format!(
            "Bad groupadd exit {}: {} {}",
            exit_status(&output.status),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn modify_group(group: GroupModDef) -> Result<(), TaskError> {
    let mut cmd = Command::new("groupmod");
    if let Some(gid) = group.gid {
        cmd.arg("-g").arg(gid.to_string());
    } else {
        return Ok(());
    }
    cmd.arg(group.name);
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(TaskError::ActionError(format!(
            "Bad groupadd exit {}: {} {}",
            exit_status(&output.status),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn group_exists(name: &str) -> Result<bool, IoError> {
    let cmd = Command::new("getent").arg("group").arg(name).output()?;
    Ok(cmd.status.success())
}

pub fn user(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_ctx, (name, opts): (String, Table)| {
        if user_exists(&name).map_err(io_error)? {
            output!("Modify user {}", name);
            modify_user(UserModDef::from_lua(name, opts)?).map_err(task_error)?;
        } else {
            output!("Create user {}", name);
            create_user(UserModDef::from_lua(name, opts)?).map_err(task_error)?;
        }
        Ok(())
    })?;
    lua.globals().set("usermod", f)?;
    Ok(())
}

pub fn user_exists_action(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_ctx, name: String| {
        let e = user_exists(&name).map_err(io_error)?;
        output!("User {} exists: {}", name, e);

        Ok(e)
    })?;
    lua.globals().set("user_exists", f)?;
    Ok(())
}

pub fn group(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|ctx, (name, opts): (String, Option<Table>)| {
        let opts = if let Some(o) = opts {
            o
        } else {
            ctx.create_table()?
        };
        if group_exists(&name).map_err(io_error)? {
            output!("Modify group {}", name);
            modify_group(GroupModDef::from_lua(name, opts)?).map_err(task_error)?;
        } else {
            output!("Create group {}", name);
            create_group(GroupModDef::from_lua(name, opts)?).map_err(task_error)?;
        }
        Ok(())
    })?;
    lua.globals().set("groupmod", f)?;
    Ok(())
}

pub fn group_exists_action(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_ctx, name: String| {
        let e = group_exists(&name).map_err(io_error)?;
        output!("Group {} exists: {}", name, e);

        Ok(e)
    })?;
    lua.globals().set("group_exists", f)?;
    Ok(())
}
