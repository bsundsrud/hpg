use std::{
    fs::{File, OpenOptions, Permissions},
    io::prelude::*,
    os::unix::prelude::PermissionsExt,
    path::Path,
};

use rlua::{Lua, Table};

use crate::{
    actions::util::{action_error, io_error, lua_table_to_json, read_file, run_chown},
    error::TaskError,
    hash, Result, WRITER,
};

fn run_template(tmpl_path: &Path, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::ActionError(format!("Invalid context: {}", e)))?;
    let tmpl_contents = read_file(tmpl_path)?;
    let rendered = tera::Tera::one_off(&tmpl_contents, &ctx, false)
        .map_err(|e| TaskError::ActionError(format!("Failed to render template: {}", e)))?;
    Ok(rendered)
}

fn hashes_match(dst: &Path, contents: &str) -> Result<bool, std::io::Error> {
    if !dst.exists() || !dst.is_file() {
        return Ok(false);
    }
    let dst_hash = hash::file_hash(&dst)?;
    let content_hash = hash::content_hash(&contents);
    Ok(dst_hash == content_hash)
}

pub fn copy(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(
            |ctx, (src, dst, options): (String, String, Option<Table>)| {
                let cwd = Path::new(".");
                let src = cwd.join(&src);
                let dst = cwd.join(&dst);
                let opts = if let Some(o) = options {
                    o
                } else {
                    ctx.create_table()?
                };

                let is_template = opts.get::<_, Option<bool>>("template")?.unwrap_or(false);
                if is_template {
                    WRITER.write(format!(
                        "render template {} to {}",
                        &src.to_string_lossy(),
                        &dst.to_string_lossy()
                    ));
                } else {
                    WRITER.write(format!(
                        "copy {} to {}",
                        &src.to_string_lossy(),
                        &dst.to_string_lossy()
                    ));
                }
                let _g = WRITER.enter("copy");
                let template_context = opts.get::<_, Option<Table>>("context")?;
                let template_context = if let Some(c) = template_context {
                    c
                } else {
                    ctx.create_table()?
                };
                let template_context = lua_table_to_json(template_context)
                    .map_err(|e| action_error(format!("Unable to parse context: {}", e)))?;
                let mode = opts.get::<_, Option<String>>("mode")?;
                let mode = mode.map(|s| {
                    u32::from_str_radix(&s, 8)
                        .map_err(|e| TaskError::ActionError(format!("Invalid Mode {}: {}", s, e)))
                });
                let user = opts.get::<_, Option<rlua::Value>>("user")?;
                let group = opts.get::<_, Option<rlua::Value>>("group")?;

                let output = if is_template {
                    run_template(&src.as_path(), template_context)
                        .map_err(|e| action_error(e.to_string()))?
                } else {
                    read_file(&src.as_path()).map_err(io_error)?
                };
                if !hashes_match(&dst, &output).map_err(io_error)? {
                    let mut outfile = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&dst)
                        .map_err(io_error)?;
                    outfile.write_all(output.as_bytes()).map_err(io_error)?;
                } else {
                    WRITER.write("files matched, skipped");
                }
                if let Some(mode) = mode {
                    let mode = mode.map_err(|e| action_error(e.to_string()))?;
                    let f = File::open(&dst).map_err(io_error)?;
                    f.set_permissions(Permissions::from_mode(mode))
                        .map_err(io_error)?;
                    WRITER.write(format!("mode: {:o}", mode));
                }
                run_chown(&dst, user, group)?;

                let retval = ctx.create_table()?;
                Ok(retval)
            },
        )?;
        lua_ctx.globals().set("copy", f)?;
        Ok(())
    })?;
    Ok(())
}
