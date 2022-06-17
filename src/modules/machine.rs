use nix::sys::utsname::uname;
use rlua::Lua;

use crate::{error::TaskError, Result};

pub fn machine(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let tbl = lua_ctx.create_table()?;
        let uname_tbl = lua_ctx.create_table()?;

        let uname_info = uname()
            .map_err(|e| TaskError::ActionError(format!("Unable to run uname, err: {}", e)))?;
        uname_tbl.set("sysname", uname_info.sysname().to_string_lossy().as_ref())?;
        uname_tbl.set("nodename", uname_info.nodename().to_string_lossy().as_ref())?;
        uname_tbl.set("release", uname_info.release().to_string_lossy().as_ref())?;
        uname_tbl.set("version", uname_info.version().to_string_lossy().as_ref())?;
        uname_tbl.set("machine", uname_info.machine().to_string_lossy().as_ref())?;
        tbl.set("uname", uname_tbl)?;
        lua_ctx.globals().set("machine", tbl)?;
        Ok(())
    })?;

    Ok(())
}
