use crate::{error, error::TaskError, Result};
use mlua::{Lua, Table};
use nix::unistd::{Uid, User as UnixUser};

#[derive(Debug)]
pub struct UserDef {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub gecos: String,
    pub home_dir: String,
    pub shell: String,
}

impl UserDef {
    fn from_unix(u: UnixUser) -> UserDef {
        UserDef {
            name: u.name.clone(),
            uid: u.uid.as_raw(),
            gid: u.gid.as_raw(),
            gecos: u.gecos.to_string_lossy().to_string(),
            home_dir: u.dir.to_string_lossy().to_string(),
            shell: u.shell.to_string_lossy().to_string(),
        }
    }

    fn to_lua<'lua>(&self, ctx: &'lua Lua) -> Result<Table<'lua>, mlua::Error> {
        let tbl = ctx.create_table()?;
        tbl.set("name", self.name.clone())?;
        tbl.set("uid", self.uid)?;
        tbl.set("gid", self.gid)?;
        tbl.set("gecos", self.gecos.clone())?;
        tbl.set("home_dir", self.home_dir.clone())?;
        tbl.set("shell", self.shell.clone())?;
        Ok(tbl)
    }
}

pub fn user(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|c, u: Option<String>| {
        let user = if let Some(username) = u {
            UnixUser::from_name(&username)
                .map_err(|e| error::action_error(format!("user syscall for {}: {}", &username, e)))?
                .ok_or_else(|| error::action_error(format!("Unknown user {}", &username)))?
        } else {
            UnixUser::from_uid(Uid::effective())
                .map_err(|e| {
                    error::action_error(format!("user syscall for current effective user: {}", e))
                })?
                .ok_or_else(|| error::action_error("Unknown current effective user"))?
        };
        let def = UserDef::from_unix(user);
        Ok(def.to_lua(&c)?)
    })?;
    lua.globals().set("user", f)?;

    Ok(())
}
