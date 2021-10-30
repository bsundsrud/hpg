use crate::error::TaskError;
use crate::WRITER;
use nix::unistd::{Gid, Group, Uid, User};
use rlua::Table;
use serde_json::Map;
use std::{convert::TryInto, fs::File, io::prelude::*, io::BufReader, path::Path, sync::Arc};

pub(crate) fn action_error<S: Into<String>>(msg: S) -> rlua::Error {
    rlua::Error::ExternalError(Arc::new(TaskError::ActionError(msg.into())))
}

pub(crate) fn task_error(err: TaskError) -> rlua::Error {
    rlua::Error::ExternalError(Arc::new(err))
}

pub(crate) fn io_error(e: std::io::Error) -> rlua::Error {
    rlua::Error::ExternalError(Arc::new(TaskError::IoError(e)))
}

pub(crate) fn read_file(path: &Path) -> Result<String, std::io::Error> {
    let mut contents = String::new();
    let f = File::open(path)?;
    let mut reader = BufReader::new(f);
    reader.read_to_string(&mut contents)?;
    Ok(contents)
}

pub(crate) fn gid_for_value(val: &rlua::Value) -> Result<Gid, rlua::Error> {
    match val {
        rlua::Value::Integer(i) => {
            let gid: u32 = (*i)
                .try_into()
                .map_err(|e| action_error(format!("gid value out of range: {}", e)))?;
            Ok(Gid::from_raw(gid))
        }

        rlua::Value::String(s) => {
            let name = s.to_str()?;
            let group = Group::from_name(name)
                .map_err(|e| action_error(format!("group: {}", e)))?
                .ok_or_else(|| action_error(format!("gid for {} not found", name)))?;
            Ok(group.gid)
        }
        _ => {
            return Err(action_error(
                "invalid group type, must be string or integer",
            ));
        }
    }
}

pub(crate) fn uid_for_value(val: &rlua::Value) -> Result<Uid, rlua::Error> {
    match val {
        rlua::Value::Integer(i) => {
            let uid: u32 = (*i)
                .try_into()
                .map_err(|e| action_error(format!("uid value out of range: {}", e)))?;
            Ok(Uid::from_raw(uid))
        }

        rlua::Value::String(s) => {
            let name = s.to_str()?;
            let user = User::from_name(name)
                .map_err(|e| action_error(format!("user: {}", e)))?
                .ok_or_else(|| action_error(format!("uid for {} not found", name)))?;
            Ok(user.uid)
        }
        _ => {
            return Err(action_error(
                "invalid group type, must be string or integer",
            ));
        }
    }
}

pub(crate) fn run_chown(
    p: &Path,
    user: Option<rlua::Value>,
    group: Option<rlua::Value>,
) -> Result<(), rlua::Error> {
    match (user, group) {
        (None, None) => {}
        (None, Some(g)) => {
            let gid = gid_for_value(&g)?;
            nix::unistd::chown(p, None, Some(gid))
                .map_err(|e| action_error(format!("chown: {}", e)))?;
            WRITER.write(format!("gid: {}", gid));
        }
        (Some(u), None) => {
            let uid = uid_for_value(&u)?;
            nix::unistd::chown(p, Some(uid), None)
                .map_err(|e| action_error(format!("chown: {}", e)))?;
            WRITER.write(format!("uid: {}", uid));
        }
        (Some(u), Some(g)) => {
            let uid = uid_for_value(&u)?;
            let gid = gid_for_value(&g)?;
            nix::unistd::chown(p, Some(uid), Some(gid))
                .map_err(|e| action_error(format!("chown: {}", e)))?;
            WRITER.write(format!("uid: {}", uid));
            WRITER.write(format!("gid: {}", gid));
        }
    }
    Ok(())
}

pub(crate) fn lua_table_to_json<'lua>(tbl: Table<'lua>) -> Result<serde_json::Value, TaskError> {
    use rlua::Value as LuaValue;
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
    Ok(serde_json::Value::Object(map))
}
