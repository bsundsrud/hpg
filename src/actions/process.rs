use std::{
    collections::HashMap,
    io::prelude::*,
    process::{ExitStatus, Stdio},
};

use rlua::{Lua, Table};
use tempfile::NamedTempFile;

use super::util::io_error;
use crate::Result;
use crate::WRITER;
use crate::{actions::util::action_error, error::TaskError};
use std::os::unix::process::ExitStatusExt;

fn exit_status(e: &ExitStatus) -> i32 {
    match e.code() {
        Some(c) => c,
        None => 127 + e.signal().unwrap_or(0),
    }
}

struct ProcessOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

fn exec_process(
    cmd: String,
    args: Vec<String>,
    inherit_env: bool,
    env: HashMap<String, String>,
    cwd: Option<String>,
    capture_stdout: bool,
    capture_stderr: bool,
) -> Result<ProcessOutput, rlua::Error> {
    let mut p = std::process::Command::new(&cmd);
    p.args(args);
    if let Some(cwd) = cwd {
        p.current_dir(cwd);
    }

    if !inherit_env {
        p.env_clear();
    }
    p.envs(env.into_iter());
    if capture_stdout {
        p.stdout(Stdio::piped());
    } else {
        p.stdout(Stdio::null());
    }
    if capture_stderr {
        p.stderr(Stdio::piped());
    } else {
        p.stderr(Stdio::null());
    }

    let output = p.output().map_err(io_error)?;
    let status = exit_status(&output.status);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok(ProcessOutput {
        status,
        stdout,
        stderr,
    })
}

pub fn shell(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (cmd, options): (String, Option<Table>)| {
            let opts = if let Some(o) = options {
                o
            } else {
                ctx.create_table()?
            };

            WRITER.write(format!("exec [ {} ]:", &cmd));

            let _guard = WRITER.enter("shell");
            let inherit_env = opts.get::<_, Option<bool>>("inherit_env")?.unwrap_or(true);
            let env = opts
                .get::<_, Option<HashMap<String, String>>>("env")?
                .unwrap_or_else(HashMap::new);
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
                .unwrap_or_else(Vec::new);

            let mut temp_file = NamedTempFile::new().map_err(io_error)?;
            temp_file.write_all(cmd.as_bytes()).map_err(io_error)?;
            let temp_path = temp_file.into_temp_path();
            sh_args.push(temp_path.to_str().unwrap().to_string());
            let output = exec_process(sh, sh_args, inherit_env, env, cwd, stdout, stderr)?;
            let retval = ctx.create_table()?;
            retval.set("status", output.status)?;
            if echo && stdout && !output.stdout.is_empty() {
                WRITER.write("stdout:");
                let _g = WRITER.enter("stdout");
                WRITER.write(&output.stdout);
            }
            if echo && stderr && !output.stderr.is_empty() {
                WRITER.write("stderr:");
                let _g = WRITER.enter("stderr");
                WRITER.write(&output.stderr);
            }
            retval.set("stdout", output.stdout)?;
            retval.set("stderr", output.stderr)?;
            WRITER.write(&format!("exit: {}", output.status));
            if !ignore_exit && output.status != 0 {
                return Err(action_error(format!(
                    "Command failed with exit code {}",
                    output.status
                )));
            }
            Ok(retval)
        })?;

        lua_ctx.globals().set("shell", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn exec(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (cmd, options): (String, Option<Table>)| {
            let opts = if let Some(o) = options {
                o
            } else {
                ctx.create_table()?
            };
            let args = opts
                .get::<_, Option<Vec<String>>>("args")?
                .unwrap_or_else(Vec::new);
            if args.is_empty() {
                WRITER.write(format!("exec [ {} ]:", &cmd));
            } else {
                let args_display = &args.join(" ");
                WRITER.write(format!("exec [ {} {} ]:", &cmd, &args_display));
            }
            let _guard = WRITER.enter("exec");
            let inherit_env = opts.get::<_, Option<bool>>("inherit_env")?.unwrap_or(true);
            let env = opts
                .get::<_, Option<HashMap<String, String>>>("env")?
                .unwrap_or_else(HashMap::new);
            let cwd: Option<String> = opts.get("cwd")?;
            let stdout = opts.get::<_, Option<bool>>("stdout")?.unwrap_or(true);
            let stderr = opts.get::<_, Option<bool>>("stderr")?.unwrap_or(true);
            let echo = opts.get::<_, Option<bool>>("echo")?.unwrap_or(true);
            let ignore_exit = opts.get::<_, Option<bool>>("ignore_exit")?.unwrap_or(false);
            let output = exec_process(cmd, args, inherit_env, env, cwd, stdout, stderr)?;
            let retval = ctx.create_table()?;
            retval.set("status", output.status)?;
            if echo && stdout && !output.stdout.is_empty() {
                WRITER.write("stdout:");
                let _g = WRITER.enter("stdout");
                WRITER.write(&output.stdout);
            }
            if echo && stderr && !output.stderr.is_empty() {
                WRITER.write("stderr:");
                let _g = WRITER.enter("stderr");
                WRITER.write(&output.stderr);
            }
            retval.set("stdout", output.stdout)?;
            retval.set("stderr", output.stderr)?;
            WRITER.write(&format!("exit: {}", output.status));
            if !ignore_exit && output.status != 0 {
                return Err(action_error(format!(
                    "Command failed with exit code {}",
                    output.status
                )));
            }
            Ok(retval)
        })?;

        lua_ctx.globals().set("exec", f)?;
        Ok(())
    })?;
    Ok(())
}
