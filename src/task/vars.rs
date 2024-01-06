use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Context};
use mlua::{Lua, MetaMethod, UserData, Value};
use serde::{Deserialize, Serialize};

use crate::{
    actions::util,
    error::{self, HpgError},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Variables {
    raw: serde_json::Value,
}

impl Variables {
    pub fn from_json(json: serde_json::Value) -> Variables {
        Variables { raw: json }
    }

    pub fn from_map(map: &HashMap<String, String>) -> Result<Variables, serde_json::Error> {
        let json = serde_json::to_value(&map)?;
        Ok(Variables::from_json(json))
    }

    pub fn from_file(f: &str) -> Result<Variables, anyhow::Error> {
        let s = crate::load_file(&f)?;
        let json = serde_json::from_str(&s).with_context(|| format!("File: {}", f))?;
        Ok(Variables::from_json(json))
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

    pub fn merge(self, other: Variables) -> Result<Variables, anyhow::Error> {
        let raw = merge_objects(self.raw, other.raw)?;
        Ok(Variables::from_json(raw))
    }
}

impl Default for Variables {
    fn default() -> Self {
        Self {
            raw: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

fn merge_objects(
    left: serde_json::Value,
    right: serde_json::Value,
) -> Result<serde_json::Value, anyhow::Error> {
    use serde_json::Value;

    match (left, right) {
        (Value::Object(mut left), Value::Object(mut right)) => {
            left.append(&mut right);
            Ok(Value::Object(left))
        }
        _ => {
            return Err(anyhow!("Only JSON Objects can be merged"));
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
