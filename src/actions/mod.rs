use std::sync::Arc;

use crate::{error::TaskError, tasks::TaskResult, Result, WRITER};
mod copy;
mod file;
mod packages;
mod process;
mod util;
pub use copy::copy;
pub use file::{mkdir, symlink, touch};
pub use packages::package;
pub use process::{exec, shell};
use rlua::Lua;

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

pub fn cancel(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, msg: Option<String>| {
            WRITER.write("cancel:");
            let _guard = WRITER.enter("cancel");
            if let Some(ref m) = msg {
                WRITER.write(&m);
            }
            Ok(TaskResult::Incomplete(msg))
        })?;
        lua_ctx.globals().set("cancel", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn success(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, msg: Option<String>| {
            WRITER.write("success:");
            let _guard = WRITER.enter("success");
            if let Some(ref m) = msg {
                WRITER.write(&m);
            }
            Ok(TaskResult::Success)
        })?;
        lua_ctx.globals().set("success", f)?;
        Ok(())
    })?;
    Ok(())
}
