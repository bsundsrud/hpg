use crate::{error::TaskError, Result};
use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Error as ReqwestError, IntoUrl, StatusCode, Url,
};
use rlua::{Lua, Table, UserData};
use std::fs::OpenOptions;

use crate::actions::util;

use super::file::HpgFile;

#[derive(Debug, Clone)]
pub struct HpgUrl {
    url: Url,
}

impl HpgUrl {
    pub fn new<U: IntoUrl>(u: U) -> Result<HpgUrl, ReqwestError> {
        Ok(HpgUrl { url: u.into_url()? })
    }
}

fn opts_to_request<'lua>(
    client: &Client,
    url: &Url,
    opts: &Table<'lua>,
) -> Result<RequestBuilder, rlua::Error> {
    let mut builder = client.get(url.clone());
    if let Some(headers) = opts.get::<_, Option<Table>>("headers")? {
        for pair in headers.pairs::<String, String>() {
            let (key, value) = pair?;
            builder = builder.header(key, value);
        }
    }
    Ok(builder)
}

fn validate_response<'lua>(resp: &Response, opts: &Table<'lua>) -> Result<(), rlua::Error> {
    let expected_response = opts
        .get::<_, Option<u16>>("expected_response")?
        .unwrap_or(200);

    if resp.status()
        != StatusCode::from_u16(expected_response).map_err(|_| {
            util::action_error(format!("Invalid expected status {}", expected_response))
        })?
    {
        return Err(util::action_error(format!(
            "Invalid response code, got {} expected {}",
            resp.status().as_u16(),
            expected_response
        )));
    }
    Ok(())
}

impl UserData for HpgUrl {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("get", |ctx, this, opts: Option<Table>| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = opts_to_request(&client, &this.url, &opts)?;
            let res = builder
                .send()
                .map_err(|e| util::action_error(format!("{}", e)))?;

            validate_response(&res, &opts)?;

            Ok(res
                .text()
                .map_err(|e| util::action_error(format!("Body error: {}", e)))?)
        });

        methods.add_method("json", |ctx, this, opts: Option<Table>| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = opts_to_request(&client, &this.url, &opts)?;
            let res = builder
                .send()
                .map_err(|e| util::action_error(format!("{}", e)))?;

            validate_response(&res, &opts)?;

            let j: serde_json::Value = res
                .json()
                .map_err(|e| util::action_error(format!("Body error: {}", e)))?;

            Ok(util::json_to_lua_value(ctx, j)?)
        });

        methods.add_method("save", |ctx, this, (dst, opts): (String, Option<Table>)| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = opts_to_request(&client, &this.url, &opts)?;
            let mut res = builder
                .send()
                .map_err(|e| util::action_error(format!("{}", e)))?;

            validate_response(&res, &opts)?;

            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&dst)
                .map_err(util::io_error)?;

            res.copy_to(&mut f)
                .map_err(|e| util::action_error(format!("Body Error: {}", e)))?;
            Ok(HpgFile::new(&dst))
        });
    }
}

pub fn url(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, u: String| {
            let u =
                HpgUrl::new(&u).map_err(|e| util::action_error(format!("Invalid Url: {}", e)))?;
            Ok(u)
        })?;
        lua_ctx.globals().set("url", f)?;
        Ok(())
    })?;
    Ok(())
}
