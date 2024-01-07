use crate::error::{self, io_error, TaskError};
use crate::tracker::Tracker;
use crate::{indent_output, tracker};

use console::style;
use mlua::{IntoLua, Lua, Table};
use nix::unistd::{Gid, Group, Uid, User};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Stdio};
use std::{convert::TryInto, fs::File, io::prelude::*, path::Path};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;

pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut contents = Vec::new();
    let f = File::open(path)?;
    let mut reader = std::io::BufReader::new(f);
    reader.read_to_end(&mut contents)?;
    Ok(contents)
}

pub(crate) fn gid_for_value(val: &mlua::Value) -> Result<Gid, mlua::Error> {
    match val {
        mlua::Value::Integer(i) => {
            let gid: u32 = (*i)
                .try_into()
                .map_err(|e| error::action_error(format!("gid value out of range: {}", e)))?;
            Ok(Gid::from_raw(gid))
        }

        mlua::Value::String(s) => {
            let name = s.to_str()?;
            let group = Group::from_name(name)
                .map_err(|e| error::action_error(format!("group: {}", e)))?
                .ok_or_else(|| error::action_error(format!("gid for {} not found", name)))?;
            Ok(group.gid)
        }
        _ => Err(error::action_error(
            "invalid group type, must be string or integer",
        )),
    }
}

pub(crate) fn uid_for_value(val: &mlua::Value) -> Result<Uid, mlua::Error> {
    match val {
        mlua::Value::Integer(i) => {
            let uid: u32 = (*i)
                .try_into()
                .map_err(|e| error::action_error(format!("uid value out of range: {}", e)))?;
            Ok(Uid::from_raw(uid))
        }

        mlua::Value::String(s) => {
            let name = s.to_str()?;
            let user = User::from_name(name)
                .map_err(|e| error::action_error(format!("user: {}", e)))?
                .ok_or_else(|| error::action_error(format!("uid for {} not found", name)))?;
            Ok(user.uid)
        }
        _ => Err(error::action_error(
            "invalid group type, must be string or integer",
        )),
    }
}

pub(crate) fn run_chown(
    p: &Path,
    user: Option<Uid>,
    group: Option<Gid>,
) -> Result<(), mlua::Error> {
    nix::unistd::chown(p, user, group)
        .map_err(|e| error::action_error(format!("chown {}: {}", p.to_string_lossy(), e)))?;
    Ok(())
}

pub(crate) fn run_chown_recursive(
    p: &Path,
    user: Option<Uid>,
    group: Option<Gid>,
) -> Result<(), mlua::Error> {
    run_chown(p, user, group)?;
    if p.is_dir() {
        for ent in std::fs::read_dir(p)? {
            let ent = ent?;
            let ty = ent.file_type()?;
            if ty.is_dir() {
                run_chown_recursive(&ent.path(), user, group)?;
            } else {
                run_chown(&ent.path(), user, group)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn lua_table_to_json(tbl: Table<'_>) -> Result<Value, TaskError> {
    use mlua::Value as LuaValue;
    use serde_json::Value as JsonValue;

    let mut map: Map<String, JsonValue> = Map::new();
    for pair in tbl.pairs::<String, LuaValue>() {
        let (k, v) = pair?;
        let json_value = match v {
            LuaValue::Nil => JsonValue::Null,
            LuaValue::Boolean(b) => JsonValue::Bool(b),
            LuaValue::LightUserData(_) => continue,
            LuaValue::Integer(i) => JsonValue::Number(i.into()),
            LuaValue::Number(n) => {
                if let Some(f) = serde_json::Number::from_f64(n) {
                    f.into()
                } else {
                    continue;
                }
            }
            LuaValue::String(s) => JsonValue::String(s.to_str()?.into()),
            LuaValue::Table(t) => lua_table_to_json(t)?,
            LuaValue::Function(_) => continue,
            LuaValue::Thread(_) => continue,
            LuaValue::UserData(_) => continue,
            LuaValue::Error(_) => continue,
        };
        map.insert(k, json_value);
    }
    Ok(Value::Object(map))
}

pub(crate) fn json_to_lua_value<'lua>(
    ctx: &'lua Lua,
    json: &Value,
) -> Result<mlua::Value<'lua>, mlua::Error> {
    use mlua::Value as LuaValue;

    let val = match json {
        Value::Null => LuaValue::Nil,
        Value::Bool(b) => LuaValue::Boolean(*b),
        Value::Number(f) => LuaValue::Number(f.as_f64().unwrap()),
        Value::String(s) => s.clone().into_lua(ctx)?,
        Value::Array(v) => {
            let tbl = ctx.create_table()?;
            let mut idx = 1;
            for item in v {
                let lua_val = json_to_lua_value(ctx, item)?;
                tbl.set(idx, lua_val)?;
                idx += 1;
            }
            LuaValue::Table(tbl)
        }
        Value::Object(obj) => {
            let tbl = ctx.create_table()?;
            for (key, val) in obj.into_iter() {
                let lua_val = json_to_lua_value(ctx, val)?;
                tbl.set(key.to_string(), lua_val)?;
            }
            LuaValue::Table(tbl)
        }
    };
    Ok(val)
}

pub(crate) fn exit_status(e: &ExitStatus) -> i32 {
    match e.code() {
        Some(c) => c,
        None => 127 + e.signal().unwrap_or(0),
    }
}

#[derive(Debug)]
pub struct ProcessOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn exec_streaming_process<A, I, S>(
    cmd: &str,
    args: I,
    inherit_env: bool,
    env: HashMap<String, String>,
    cwd: Option<A>,
    capture_stdout: bool,
    capture_stderr: bool,
    echo: bool,
) -> Result<ProcessOutput, mlua::Error>
where
    A: AsRef<Path>,
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut p = tokio::process::Command::new(cmd);
    p.args(args);
    if let Some(cwd) = cwd {
        p.current_dir(cwd);
    }

    if !inherit_env {
        p.env_clear();
    }
    p.envs(env);

    p.stdout(Stdio::piped());
    p.stderr(Stdio::piped());
    p.stdin(Stdio::piped());
    tracker::tracker().suspend_bars();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let handle = rt.handle();
        let _ = handle.enter();
        let output = rt.block_on(async move {
            let mut child = p.spawn().map_err(io_error)?;

            let mut out_reader =
                BufReader::new(child.stdout.take().expect("Could not open stdout on child"))
                    .lines();
            let mut err_reader =
                BufReader::new(child.stderr.take().expect("Could not open stderr on child"))
                    .lines();

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
    });
    let res = handle.join().unwrap();
    tracker::tracker().resume_bars();
    res
}

#[allow(dead_code)]
fn exec_blocking_process(
    cmd: String,
    args: Vec<String>,
    inherit_env: bool,
    env: HashMap<String, String>,
    cwd: Option<String>,
    capture_stdout: bool,
    capture_stderr: bool,
) -> Result<ProcessOutput, mlua::Error> {
    let mut p = std::process::Command::new(cmd);
    p.args(args);
    if let Some(cwd) = cwd {
        p.current_dir(cwd);
    }

    if !inherit_env {
        p.env_clear();
    }
    p.envs(env);
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
