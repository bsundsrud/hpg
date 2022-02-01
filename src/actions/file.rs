use rlua::Lua;

use crate::actions::util::action_error;

use crate::error::TaskError;
use crate::{hash, Result, WRITER};

use super::util;
pub fn hash_text(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, text: String| {
            let h = hash::content_hash(&text);
            Ok(h)
        })?;
        lua_ctx.globals().set("hash", f)?;
        Ok(())
    })?;
    Ok(())
}
pub fn from_json(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, json_str: String| {
            WRITER.write("from_json");
            WRITER.enter("from_json");
            let json: serde_json::Value =
                serde_json::from_str(&json_str).map_err(|e| action_error(format!("{}", e)))?;
            let lua_val = util::json_to_lua_value(ctx, json)?;
            Ok(lua_val)
        })?;
        lua_ctx.globals().set("from_json", f)?;
        Ok(())
    })?;
    Ok(())
}
