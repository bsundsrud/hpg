use std::sync::Arc;

use crate::{error::TaskError, task::TaskResult, Result, WRITER};
mod access;
mod file;
mod process;
pub(crate) mod util;
pub use access::{group, group_exists_action, user, user_exists_action};
pub use file::{from_json, hash_text};
use mlua::{Function, Lua};
pub use process::{exec, shell};

fn format_lua_value(ctx: &Lua, v: mlua::Value) -> Result<String, mlua::Error> {
    let s = match v {
        mlua::Value::Nil => String::from("nil"),
        mlua::Value::Boolean(b) => String::from(if b { "true" } else { "false" }),
        mlua::Value::LightUserData(v) => format!("<{:?}>", v),
        mlua::Value::Integer(i) => format!("{:?}", i),
        mlua::Value::Number(n) => format!("{:?}", n),
        mlua::Value::String(s) => format!("\"{}\"", &s.to_str()?),
        mlua::Value::Table(t) => {
            let mut pairs = Vec::new();
            for pair in t.pairs() {
                let (k, v) = pair?;
                let k_str = format_lua_value(ctx, k)?;
                let v_str = format_lua_value(ctx, v)?;
                pairs.push(format!("{} = {}", k_str, v_str));
            }
            format!("{{ {} }}", pairs.join(", "))
        }
        mlua::Value::Function(f) => format!("<{:?}>", f),
        mlua::Value::Thread(t) => format!("<{:?}>", t),
        mlua::Value::UserData(d) => {
            let globals = ctx.globals();
            let tostring: Function = globals.get("tostring")?;
            let s = tostring.call::<_, String>(d)?;
            format!("\"{}\"", s)
        }
        mlua::Value::Error(e) => format!("<error: {}>", e),
    };
    Ok(s)
}

pub fn echo(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|ctx: &Lua, msg: mlua::Value| {
        WRITER.write("echo:");
        let _guard = WRITER.enter("echo");
        WRITER.write(format_lua_value(ctx, msg)?);
        Ok(())
    })?;
    lua.globals().set("echo", f)?;

    Ok(())
}

pub fn fail(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function::<_, (), _>(|_, msg: String| {
        WRITER.write("fail:");
        let _guard = WRITER.enter("fail");
        WRITER.write(&msg);
        Err(mlua::Error::ExternalError(Arc::new(
            TaskError::ActionError(msg),
        )))
    })?;
    lua.globals().set("fail", f)?;
    Ok(())
}

pub fn cancel(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, msg: Option<String>| {
        WRITER.write("cancel:");
        let _guard = WRITER.enter("cancel");
        if let Some(ref m) = msg {
            WRITER.write(&m);
        }
        Ok(TaskResult::Incomplete(msg))
    })?;
    lua.globals().set("cancel", f)?;
    Ok(())
}

pub fn success(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, msg: Option<String>| {
        WRITER.write("success:");
        let _guard = WRITER.enter("success");
        if let Some(ref m) = msg {
            WRITER.write(&m);
        }
        Ok(TaskResult::Success)
    })?;
    lua.globals().set("success", f)?;
    Ok(())
}
