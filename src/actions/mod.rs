use std::sync::Arc;

use crate::{error::TaskError, tasks::TaskResult, Result, WRITER};
mod access;
mod copy;
mod file;
mod packages;
mod process;
mod util;
pub use access::{group, user};
pub use copy::{append, copy};
pub use file::{hash_file, hash_text, mkdir, symlink, touch};
pub use packages::package;
pub use process::{exec, shell};
use rlua::Lua;

fn format_lua_value(v: rlua::Value) -> Result<String, rlua::Error> {
    let s = match v {
        rlua::Value::Nil => String::from("nil"),
        rlua::Value::Boolean(b) => String::from(if b { "true" } else { "false" }),
        rlua::Value::LightUserData(v) => format!("<{:?}>", v),
        rlua::Value::Integer(i) => format!("{:?}", i),
        rlua::Value::Number(n) => format!("{:?}", n),
        rlua::Value::String(s) => format!("\"{}\"", &s.to_str()?),
        rlua::Value::Table(t) => {
            let mut pairs = Vec::new();
            for pair in t.pairs() {
                let (k, v) = pair?;
                let k_str = format_lua_value(k)?;
                let v_str = format_lua_value(v)?;
                pairs.push(format!("{} = {}", k_str, v_str));
            }
            format!("{{ {} }}", pairs.join(", "))
        }
        rlua::Value::Function(f) => format!("<{:?}>", f),
        rlua::Value::Thread(t) => format!("<{:?}>", t),
        rlua::Value::UserData(d) => format!("<{:?}>", d),
        rlua::Value::Error(e) => format!("<error: {}>", e),
    };
    Ok(s)
}

pub fn echo(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, msg: rlua::Value| {
            WRITER.write("echo:");
            let _guard = WRITER.enter("echo");
            WRITER.write(format_lua_value(msg)?);
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
