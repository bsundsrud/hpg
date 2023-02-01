use crate::{
    error::{self, TaskError},
    Result, WRITER,
};
use mlua::{Lua, Table, UserData};
use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Error as ReqwestError, IntoUrl, StatusCode, Url,
};
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

    pub fn opts_to_request<'lua>(
        &self,
        client: &Client,
        opts: &Table<'lua>,
    ) -> Result<RequestBuilder, mlua::Error> {
        let mut builder = client.get(self.url.clone());
        if let Some(headers) = opts.get::<_, Option<Table>>("headers")? {
            for pair in headers.pairs::<String, String>() {
                let (key, value) = pair?;
                builder = builder.header(key, value);
            }
        }
        Ok(builder)
    }

    pub fn validate_response<'lua>(resp: &Response, opts: &Table<'lua>) -> Result<(), mlua::Error> {
        let expected_response = opts
            .get::<_, Option<u16>>("expected_response")?
            .unwrap_or(200);

        if resp.status()
            != StatusCode::from_u16(expected_response).map_err(|_| {
                error::action_error(format!("Invalid expected status {}", expected_response))
            })?
        {
            return Err(error::action_error(format!(
                "Invalid response code, got {} expected {}",
                resp.status().as_u16(),
                expected_response
            )));
        }
        Ok(())
    }
}

impl UserData for HpgUrl {
    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("get", |ctx, this, opts: Option<Table>| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = this.opts_to_request(&client, &opts)?;
            WRITER.write(format!("GET {}", &this.url));
            let _g = WRITER.enter("net_get");
            let res = builder
                .send()
                .map_err(|e| error::action_error(format!("{}", e)))?;

            HpgUrl::validate_response(&res, &opts)?;

            Ok(res
                .text()
                .map_err(|e| error::action_error(format!("Body error: {}", e)))?)
        });

        methods.add_method("json", |ctx, this, opts: Option<Table>| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = this.opts_to_request(&client, &opts)?;
            WRITER.write(format!("GET JSON {}", &this.url));
            let _g = WRITER.enter("net_json");
            let res = builder
                .send()
                .map_err(|e| error::action_error(format!("{}", e)))?;

            HpgUrl::validate_response(&res, &opts)?;

            let j: serde_json::Value = res
                .json()
                .map_err(|e| error::action_error(format!("Body error: {}", e)))?;

            Ok(util::json_to_lua_value(ctx, &j)?)
        });

        methods.add_method("save", |ctx, this, (dst, opts): (String, Option<Table>)| {
            let client = reqwest::blocking::Client::new();
            let opts = if let Some(o) = opts {
                o
            } else {
                ctx.create_table()?
            };
            let builder = this.opts_to_request(&client, &opts)?;
            WRITER.write(format!("Download {} to  {}", &this.url, &dst));
            let _g = WRITER.enter("net_save");
            let mut res = builder
                .send()
                .map_err(|e| error::action_error(format!("{}", e)))?;

            HpgUrl::validate_response(&res, &opts)?;

            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&dst)
                .map_err(error::io_error)?;

            res.copy_to(&mut f)
                .map_err(|e| error::action_error(format!("Body Error: {}", e)))?;
            Ok(HpgFile::new(&dst))
        });
    }
}

pub fn url(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, u: String| {
        let u = HpgUrl::new(&u).map_err(|e| error::action_error(format!("Invalid Url: {}", e)))?;
        Ok(u)
    })?;
    lua.globals().set("url", f)?;
    Ok(())
}
