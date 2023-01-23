use crate::{actions::util, error::TaskError, Result};
use nix::unistd::{Uid, User as UnixUser};
use rlua::{Context, Lua, Table};

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

    fn to_lua<'lua>(&self, ctx: Context<'lua>) -> Result<Table<'lua>, rlua::Error> {
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

pub fn user(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|ctx| {
        let f = ctx.create_function(|c, u: Option<String>| {
            let user = if let Some(username) = u {
                UnixUser::from_name(&username)
                    .map_err(|e| {
                        util::action_error(format!("user syscall for {}: {}", &username, e))
                    })?
                    .ok_or_else(|| util::action_error(format!("Unknown user {}", &username)))?
            } else {
                UnixUser::from_uid(Uid::effective())
                    .map_err(|e| {
                        util::action_error(format!(
                            "user syscall for current effective user: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| util::action_error("Unknown current effective user"))?
            };
            let def = UserDef::from_unix(user);
            Ok(def.to_lua(c.clone())?)
        })?;
        ctx.globals().set("user", f)?;

        Ok(())
    })?;

    Ok(())
}
