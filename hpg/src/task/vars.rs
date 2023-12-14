use mlua::{Lua, MetaMethod, UserData, Value};

use crate::{
    actions::util,
    error::{self, HpgError},
};

#[derive(Debug)]
pub struct Variables {
    raw: serde_json::Value,
}

impl Variables {
    pub fn from_json(json: serde_json::Value) -> Variables {
        Variables { raw: json }
    }

    fn get_from_raw(&self, key: &str) -> Result<Option<&serde_json::Value>, mlua::Error> {
        if let serde_json::Value::Object(ref o) = self.raw {
            Ok(o.get(key))
        } else {
            Err(error::action_error(
                "Invalid variables type, must be a JSON Object".to_string(),
            ))
        }
    }

    fn get_from_registry<'lua>(
        &self,
        ctx: &'lua Lua,
        key: &str,
    ) -> Result<Option<mlua::Value<'lua>>, mlua::Error> {
        let val: Option<mlua::Value> = ctx.named_registry_value(key)?;
        Ok(val)
    }

    pub fn get<'lua>(&self, ctx: &'lua Lua, key: &str) -> Result<mlua::Value<'lua>, mlua::Error> {
        let val = if let Some(v) = self.get_from_raw(key)? {
            util::json_to_lua_value(ctx, v)?
        } else if let Some(v) = self.get_from_registry(ctx, key)? {
            v
        } else {
            return Err(error::action_error(format!(
                "Variable '{}' not defined.",
                key
            )));
        };
        Ok(val)
    }

    pub fn set_default(
        &mut self,
        ctx: &Lua,
        key: &str,
        val: mlua::Value,
    ) -> Result<(), mlua::Error> {
        ctx.set_named_registry_value(key, val)?;
        Ok(())
    }

    pub fn merge(self, other: Variables) -> Result<Variables, HpgError> {
        let raw = merge_objects(self.raw, other.raw)?;
        Ok(Variables::from_json(raw))
    }
}

fn merge_objects(
    left: serde_json::Value,
    right: serde_json::Value,
) -> Result<serde_json::Value, HpgError> {
    use serde_json::Value;

    match (left, right) {
        (Value::Object(mut left), Value::Object(mut right)) => {
            left.append(&mut right);
            Ok(Value::Object(left))
        }
        _ => {
            return Err(HpgError::Parse(
                "Only JSON Objects can be merged".to_string(),
            ));
        }
    }
}

impl UserData for Variables {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |ctx, this, idx: String| {
            let v = this.get(ctx, &idx)?;
            Ok(v)
        });
        methods.add_meta_method_mut(
            MetaMethod::NewIndex,
            |ctx, this, (idx, val): (String, Value)| {
                this.set_default(ctx, &idx, val)?;
                Ok(())
            },
        );
    }
}
