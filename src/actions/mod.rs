use crate::{error::TaskError, Result};
use rlua::Lua;

pub fn echo(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, msg: String| {
            println!("{}", msg);
            Ok(())
        })?;
        lua_ctx.globals().set("echo", f)?;
        Ok(())
    })?;
    Ok(())
}
