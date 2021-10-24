use std::sync::Arc;

use rlua::Lua;

use crate::{error::TaskError, Result, WRITER};

pub fn echo(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, msg: String| {
            WRITER.write("echo:");
            let _guard = WRITER.enter("echo");
            WRITER.write(msg);
            Ok(())
        })?;
        lua_ctx.globals().set("echo", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn fail(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function::<_, (), _>(|_, msg: String| {
            WRITER.write("fail:");
            let _guard = WRITER.enter("fail");
            WRITER.write(&msg);
            Err(rlua::Error::ExternalError(Arc::new(
                TaskError::ActionError(msg),
            )))
        })?;
        lua_ctx.globals().set("fail", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn exec(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, args: Table| todo!())?;

        lua_ctx.globals().set("exec", f)?;
        Ok(())
    })?;
    Ok(())
}
