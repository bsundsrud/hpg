use std::fs::{self, File, OpenOptions, Permissions};
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;

use rlua::{Lua, Table};

use crate::actions::util::{action_error, io_error, run_chown};

use crate::error::TaskError;
use crate::{Result, WRITER};

pub fn symlink(lua: &Lua) -> Result<()> {
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
                WRITER.write(format!(
                    "Symlink {} to {}",
                    &src.to_string_lossy(),
                    &dst.to_string_lossy()
                ));
                WRITER.enter("symlink");
                let mode = opts.get::<_, Option<String>>("mode")?;
                let mode = mode.map(|s| {
                    u32::from_str_radix(&s, 8)
                        .map_err(|e| TaskError::ActionError(format!("Invalid Mode {}: {}", s, e)))
                });
                let user = opts.get::<_, Option<rlua::Value>>("user")?;
                let group = opts.get::<_, Option<rlua::Value>>("group")?;

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
        lua_ctx.globals().set("symlink", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn mkdir(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (path, options): (String, Option<Table>)| {
            let cwd = Path::new(".");
            let p = cwd.join(path);
            let opts = if let Some(o) = options {
                o
            } else {
                ctx.create_table()?
            };
            WRITER.write(format!("mkdir {}", p.to_string_lossy()));
            WRITER.enter("mkdir");
            let mode = opts.get::<_, Option<String>>("mode")?;
            let mode = mode.map(|s| {
                u32::from_str_radix(&s, 8)
                    .map_err(|e| TaskError::ActionError(format!("Invalid Mode {}: {}", s, e)))
            });
            let user = opts.get::<_, Option<rlua::Value>>("user")?;
            let group = opts.get::<_, Option<rlua::Value>>("group")?;

            fs::create_dir_all(&p).map_err(io_error)?;

            if let Some(mode) = mode {
                let mode = mode.map_err(|e| action_error(e.to_string()))?;
                let f = File::open(&p).map_err(io_error)?;
                f.set_permissions(Permissions::from_mode(mode))
                    .map_err(io_error)?;
                WRITER.write(format!("mode: {:o}", mode));
            }
            run_chown(&p, user, group)?;

            Ok(p.to_string_lossy().into_owned())
        })?;
        lua_ctx.globals().set("mkdir", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn touch(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (path, options): (String, Option<Table>)| {
            let cwd = Path::new(".");
            let p = cwd.join(path);
            let opts = if let Some(o) = options {
                o
            } else {
                ctx.create_table()?
            };
            WRITER.write(format!("touch {}", p.to_string_lossy()));
            WRITER.enter("touch");
            let mode = opts.get::<_, Option<String>>("mode")?;
            let mode = mode.map(|s| {
                u32::from_str_radix(&s, 8)
                    .map_err(|e| TaskError::ActionError(format!("Invalid Mode {}: {}", s, e)))
            });
            let user = opts.get::<_, Option<rlua::Value>>("user")?;
            let group = opts.get::<_, Option<rlua::Value>>("group")?;

            let f = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&p)
                .map_err(io_error)?;

            if let Some(mode) = mode {
                let mode = mode.map_err(|e| action_error(e.to_string()))?;
                f.set_permissions(Permissions::from_mode(mode))
                    .map_err(io_error)?;
                WRITER.write(format!("mode: {:o}", mode));
            }
            drop(f);

            run_chown(&p, user, group)?;

            Ok(p.to_string_lossy().into_owned())
        })?;
        lua_ctx.globals().set("touch", f)?;
        Ok(())
    })?;
    Ok(())
}
