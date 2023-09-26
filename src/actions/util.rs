use crate::error::{self, TaskError};
use crate::output;

use mlua::{IntoLua, Lua, Table};
use nix::unistd::{Gid, Group, Uid, User};
use serde_json::{Map, Value};
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::{convert::TryInto, fs::File, io::prelude::*, io::BufReader, path::Path};

pub(crate) fn read_file(path: &Path) -> Result<String, std::io::Error> {
    let mut contents = String::new();
    let f = File::open(path)?;
    let mut reader = BufReader::new(f);
    reader.read_to_string(&mut contents)?;
    Ok(contents)
}

pub(crate) fn gid_for_value(val: &mlua::Value) -> Result<Gid, mlua::Error> {
    match val {
        mlua::Value::Integer(i) => {
            let gid: u32 = (*i)
                .try_into()
                .map_err(|e| error::action_error(format!("gid value out of range: {}", e)))?;
            Ok(Gid::from_raw(gid))
        }

        mlua::Value::String(s) => {
            let name = s.to_str()?;
            let group = Group::from_name(name)
                .map_err(|e| error::action_error(format!("group: {}", e)))?
                .ok_or_else(|| error::action_error(format!("gid for {} not found", name)))?;
            Ok(group.gid)
        }
        _ => {
            return Err(error::action_error(
                "invalid group type, must be string or integer",
            ));
        }
    }
}

pub(crate) fn uid_for_value(val: &mlua::Value) -> Result<Uid, mlua::Error> {
    match val {
        mlua::Value::Integer(i) => {
            let uid: u32 = (*i)
                .try_into()
                .map_err(|e| error::action_error(format!("uid value out of range: {}", e)))?;
            Ok(Uid::from_raw(uid))
        }

        mlua::Value::String(s) => {
            let name = s.to_str()?;
            let user = User::from_name(name)
                .map_err(|e| error::action_error(format!("user: {}", e)))?
                .ok_or_else(|| error::action_error(format!("uid for {} not found", name)))?;
            Ok(user.uid)
        }
        _ => {
            return Err(error::action_error(
                "invalid group type, must be string or integer",
            ));
        }
    }
}

pub(crate) fn run_chown(
    p: &Path,
    user: Option<mlua::Value>,
    group: Option<mlua::Value>,
) -> Result<(), mlua::Error> {
    match (user, group) {
        (None, None) => {}
        (None, Some(g)) => {
            let gid = gid_for_value(&g)?;
            nix::unistd::chown(p, None, Some(gid))
                .map_err(|e| error::action_error(format!("chown: {}", e)))?;
            output!("gid: {}", gid);
        }
        (Some(u), None) => {
            let uid = uid_for_value(&u)?;
            nix::unistd::chown(p, Some(uid), None)
                .map_err(|e| error::action_error(format!("chown: {}", e)))?;
            output!("uid: {}", uid);
        }
        (Some(u), Some(g)) => {
            let uid = uid_for_value(&u)?;
            let gid = gid_for_value(&g)?;
            nix::unistd::chown(p, Some(uid), Some(gid))
                .map_err(|e| error::action_error(format!("chown: {}", e)))?;
            output!("uid: {}", uid);
            output!("gid: {}", gid);
        }
    }
    Ok(())
}

pub(crate) fn lua_table_to_json<'lua>(tbl: Table<'lua>) -> Result<Value, TaskError> {
    use mlua::Value as LuaValue;
    use serde_json::Value as JsonValue;

    let mut map: Map<String, JsonValue> = Map::new();
    for pair in tbl.pairs::<String, LuaValue>() {
        let (k, v) = pair?;
        let json_value = match v {
            LuaValue::Nil => JsonValue::Null,
            LuaValue::Boolean(b) => JsonValue::Bool(b),
            LuaValue::LightUserData(_) => continue,
            LuaValue::Integer(i) => JsonValue::Number(i.into()),
            LuaValue::Number(n) => {
                if let Some(f) = serde_json::Number::from_f64(n) {
                    f.into()
                } else {
                    continue;
                }
            }
            LuaValue::String(s) => JsonValue::String(s.to_str()?.into()),
            LuaValue::Table(t) => lua_table_to_json(t)?,
            LuaValue::Function(_) => continue,
            LuaValue::Thread(_) => continue,
            LuaValue::UserData(_) => continue,
            LuaValue::Error(_) => continue,
        };
        map.insert(k, json_value);
    }
    Ok(Value::Object(map))
}

pub(crate) fn json_to_lua_value<'lua>(
    ctx: &'lua Lua,
    json: &Value,
) -> Result<mlua::Value<'lua>, mlua::Error> {
    use mlua::Value as LuaValue;

    let val = match json {
        Value::Null => LuaValue::Nil,
        Value::Bool(b) => LuaValue::Boolean(*b),
        Value::Number(f) => LuaValue::Number(f.as_f64().unwrap()),
        Value::String(s) => s.clone().into_lua(ctx)?,
        Value::Array(v) => {
            let tbl = ctx.create_table()?;
            let mut idx = 1;
            for item in v {
                let lua_val = json_to_lua_value(ctx, item)?;
                tbl.set(idx, lua_val)?;
                idx += 1;
            }
            LuaValue::Table(tbl)
        }
        Value::Object(obj) => {
            let tbl = ctx.create_table()?;
            for (key, val) in obj.into_iter() {
                let lua_val = json_to_lua_value(ctx, val)?;
                tbl.set(key.to_string(), lua_val)?;
            }
            LuaValue::Table(tbl)
        }
    };
    Ok(val)
}

pub(crate) fn exit_status(e: &ExitStatus) -> i32 {
    match e.code() {
        Some(c) => c,
        None => 127 + e.signal().unwrap_or(0),
    }
}
