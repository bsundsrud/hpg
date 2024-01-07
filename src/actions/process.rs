use std::{collections::HashMap, io::prelude::*};

use mlua::{Lua, Table};
use tempfile::NamedTempFile;

use crate::actions::util::exec_streaming_process;
use crate::error::{action_error, io_error, TaskError};
use crate::{indent_output, output, Result};

pub fn shell(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|ctx, (cmd, options): (String, Option<Table>)| {
        let opts = if let Some(o) = options {
            o
        } else {
            ctx.create_table()?
        };

        output!("exec [ {} ]:", &cmd);

        let inherit_env = opts.get::<_, Option<bool>>("inherit_env")?.unwrap_or(true);
        let env = opts
            .get::<_, Option<HashMap<String, String>>>("env")?
            .unwrap_or_default();
        let cwd: Option<String> = opts.get("cwd")?;
        let stdout = opts.get::<_, Option<bool>>("stdout")?.unwrap_or(true);
        let stderr = opts.get::<_, Option<bool>>("stderr")?.unwrap_or(true);
        let echo = opts.get::<_, Option<bool>>("echo")?.unwrap_or(true);
        let ignore_exit = opts.get::<_, Option<bool>>("ignore_exit")?.unwrap_or(false);
        let sh = opts
            .get::<_, Option<String>>("sh")?
            .unwrap_or_else(|| String::from("/bin/sh"));
        let mut sh_args = opts
            .get::<_, Option<Vec<String>>>("sh_args")?
            .unwrap_or_default();

        let mut temp_file = NamedTempFile::new().map_err(io_error)?;
        temp_file.write_all(cmd.as_bytes()).map_err(io_error)?;
        let temp_path = temp_file.into_temp_path();
        sh_args.push(temp_path.to_str().unwrap().to_string());
        let output =
            exec_streaming_process(&sh, &sh_args, inherit_env, env, cwd, stdout, stderr, echo)?;

        let retval = ctx.create_table()?;
        retval.set("status", output.status)?;
        retval.set("stdout", output.stdout)?;
        retval.set("stderr", output.stderr)?;
        indent_output!(1, "exit: {}", output.status);
        if !ignore_exit && output.status != 0 {
            return Err(action_error(format!(
                "Command failed with exit code {}",
                output.status
            )));
        }
        Ok(retval)
    })?;

    lua.globals().set("shell", f)?;
    Ok(())
}

pub fn exec(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|ctx, (cmd, options): (String, Option<Table>)| {
        let opts = if let Some(o) = options {
            o
        } else {
            ctx.create_table()?
        };
        let args = opts
            .get::<_, Option<Vec<String>>>("args")?
            .unwrap_or_default();
        if args.is_empty() {
            output!("exec [ {} ]:", &cmd);
        } else {
            let args_display = &args.join(" ");
            output!("exec [ {} {} ]:", &cmd, &args_display);
        }
        let inherit_env = opts.get::<_, Option<bool>>("inherit_env")?.unwrap_or(true);
        let env = opts
            .get::<_, Option<HashMap<String, String>>>("env")?
            .unwrap_or_default();
        let cwd: Option<String> = opts.get("cwd")?;
        let stdout = opts.get::<_, Option<bool>>("stdout")?.unwrap_or(true);
        let stderr = opts.get::<_, Option<bool>>("stderr")?.unwrap_or(true);
        let echo = opts.get::<_, Option<bool>>("echo")?.unwrap_or(true);
        let ignore_exit = opts.get::<_, Option<bool>>("ignore_exit")?.unwrap_or(false);
        let output =
            exec_streaming_process(&cmd, args, inherit_env, env, cwd, stdout, stderr, echo)?;
        let retval = ctx.create_table()?;
        retval.set("status", output.status)?;
        retval.set("stdout", output.stdout)?;
        retval.set("stderr", output.stderr)?;
        output!("  exit: {}", output.status);
        if !ignore_exit && output.status != 0 {
            return Err(action_error(format!(
                "Command failed with exit code {}",
                output.status
            )));
        }
        Ok(retval)
    })?;

    lua.globals().set("exec", f)?;
    Ok(())
}
