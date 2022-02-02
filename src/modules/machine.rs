use nix::sys::utsname::uname;
use rlua::Lua;

use crate::{error::TaskError, Result};

pub fn machine(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let tbl = lua_ctx.create_table()?;
        let uname_tbl = lua_ctx.create_table()?;

        let uname_info = uname();
        uname_tbl.set("sysname", uname_info.sysname())?;
        uname_tbl.set("nodename", uname_info.nodename())?;
        uname_tbl.set("release", uname_info.release())?;
        uname_tbl.set("version", uname_info.version())?;
        uname_tbl.set("machine", uname_info.machine())?;
        tbl.set("uname", uname_tbl)?;
        lua_ctx.globals().set("machine", tbl)?;
        Ok(())
    })?;

    Ok(())
}
