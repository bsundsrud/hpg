use std::{collections::HashMap, io::prelude::*, process::Stdio};

use console::style;
use mlua::{Lua, Table};
use tempfile::NamedTempFile;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::runtime::Builder;
use tokio::select;

use super::util::exit_status;
use crate::error::{action_error, io_error, TaskError};
use crate::{indent_output, output, Result};

struct ProcessOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

fn exec_streaming_process(
    cmd: String,
    args: Vec<String>,
    inherit_env: bool,
    env: HashMap<String, String>,
    cwd: Option<String>,
    capture_stdout: bool,
    capture_stderr: bool,
    echo: bool,
) -> Result<ProcessOutput, mlua::Error> {
    let mut p = tokio::process::Command::new(&cmd);
    p.args(args);
    if let Some(cwd) = cwd {
        p.current_dir(cwd);
    }

    if !inherit_env {
        p.env_clear();
    }
    p.envs(env.into_iter());

    p.stdout(Stdio::piped());
    p.stderr(Stdio::piped());
    p.stdin(Stdio::null());

    let rt = Builder::new_multi_thread()
        .enable_all()
        .thread_name("process-exec")
        .build()
        .expect("Could not build runtime");
    let handle = rt.handle().clone();
    let output = rt.block_on(async move {
        let mut child = p.spawn().map_err(io_error)?;

        let mut out_reader =
            BufReader::new(child.stdout.take().expect("Could not open stdout on child")).lines();
        let mut err_reader =
            BufReader::new(child.stderr.take().expect("Could not open stderr on child")).lines();

        let join_handle = handle.spawn(async move { child.wait().await.map_err(io_error) });
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        loop {
            select! {
                maybe_line = out_reader.next_line() => {
                    if let Some(line) = maybe_line? {
                        if capture_stdout {
                            if echo {
                                indent_output!(1, "{}", line);
                            }
                            stdout_lines.push(line);
                        }
                    } else {
                        break;
                    }
                },
                maybe_line = err_reader.next_line() => {
                    if let Some(line) = maybe_line? {
                        if capture_stderr {
                            if echo {
                                indent_output!(1, "{}", style(&line).yellow());
                            }
                            stderr_lines.push(line);
                        }
                    } else {
                        break;
                    }
                },
            }
        }
        let res = join_handle.await.expect("Failed to join child")?;
        let status = exit_status(&res);
        let stdout = stdout_lines.join("\n");
        let stderr = stderr_lines.join("\n");
        Ok::<_, mlua::Error>(ProcessOutput {
            status,
            stdout,
            stderr,
        })
    })?;
    Ok(output)
}

fn exec_blocking_process(
    cmd: String,
    args: Vec<String>,
    inherit_env: bool,
    env: HashMap<String, String>,
    cwd: Option<String>,
    capture_stdout: bool,
    capture_stderr: bool,
) -> Result<ProcessOutput, mlua::Error> {
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

        let output =
            exec_streaming_process(sh, sh_args, inherit_env, env, cwd, stdout, stderr, echo)?;

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
            .unwrap_or_else(Vec::new);
        if args.is_empty() {
            output!("exec [ {} ]:", &cmd);
        } else {
            let args_display = &args.join(" ");
            output!("exec [ {} {} ]:", &cmd, &args_display);
        }
        let inherit_env = opts.get::<_, Option<bool>>("inherit_env")?.unwrap_or(true);
        let env = opts
            .get::<_, Option<HashMap<String, String>>>("env")?
            .unwrap_or_else(HashMap::new);
        let cwd: Option<String> = opts.get("cwd")?;
        let stdout = opts.get::<_, Option<bool>>("stdout")?.unwrap_or(true);
        let stderr = opts.get::<_, Option<bool>>("stderr")?.unwrap_or(true);
        let echo = opts.get::<_, Option<bool>>("echo")?.unwrap_or(true);
        let ignore_exit = opts.get::<_, Option<bool>>("ignore_exit")?.unwrap_or(false);
        let output =
            exec_streaming_process(cmd, args, inherit_env, env, cwd, stdout, stderr, echo)?;
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
