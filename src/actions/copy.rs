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

fn run_template(tmpl: &str, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::ActionError(format!("Invalid context: {}", e)))?;
    let rendered = tera::Tera::one_off(&tmpl, &ctx, false)
        .map_err(|e| TaskError::ActionError(format!("Failed to render template: {}", e)))?;
    Ok(rendered)
}

fn run_template_file(tmpl_path: &Path, context: serde_json::Value) -> Result<String, TaskError> {
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
                    run_template_file(&src.as_path(), template_context)
                        .map_err(|e| action_error(e.to_string()))?
                } else {
                    read_file(&src.as_path()).map_err(io_error)?
                };
                let updated;
                if !hashes_match(&dst, &output).map_err(io_error)? {
                    let mut outfile = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&dst)
                        .map_err(io_error)?;
                    outfile.write_all(output.as_bytes()).map_err(io_error)?;
                    updated = true;
                } else {
                    WRITER.write("files matched, skipped");
                    updated = false;
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
                retval.set("updated", updated)?;
                Ok(retval)
            },
        )?;
        lua_ctx.globals().set("copy", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn append(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|ctx, (dst, options): (String, Option<Table>)| {
            let cwd = Path::new(".");
            let dst = cwd.join(&dst);
            let opts = if let Some(o) = options {
                o
            } else {
                ctx.create_table()?
            };
            let src = opts.get::<_, Option<String>>("src")?;
            let contents = opts.get::<_, Option<String>>("contents")?;
            let input = match (src, contents) {
                (None, None) => {
                    return Err(action_error(
                        "append: Must specify one of 'src' or 'contents'",
                    ))
                }
                (None, Some(s)) => s,
                (Some(path), None) => read_file(Path::new(&path)).map_err(io_error)?,
                (Some(_), Some(_)) => {
                    return Err(action_error(
                        "append: Must specify only one of 'src' or 'contents'",
                    ))
                }
            };
            let marker = opts
                .get::<_, Option<String>>("marker")?
                .ok_or_else(|| action_error("append: 'marker' is required"))?;
            let is_template = opts.get::<_, Option<bool>>("template")?.unwrap_or(false);
            if is_template {
                WRITER.write(format!("append template to {}", &dst.to_string_lossy()));
            } else {
                WRITER.write(format!("append to {}", &dst.to_string_lossy()));
            }
            let _g = WRITER.enter("append");
            let template_context = opts.get::<_, Option<Table>>("context")?;
            let template_context = if let Some(c) = template_context {
                c
            } else {
                ctx.create_table()?
            };
            let template_context = lua_table_to_json(template_context)
                .map_err(|e| action_error(format!("Unable to parse context: {}", e)))?;

            let output = if is_template {
                run_template(&input, template_context).map_err(|e| action_error(e.to_string()))?
            } else {
                input
            };
            let content_hash = hash::content_hash(&output);
            let updated;
            if !dst.exists() {
                let contents = format!(
                    "{} {}\n{}\n{} {}\n",
                    marker, content_hash, output, marker, content_hash
                );
                let mut outfile = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&dst)
                    .map_err(io_error)?;
                outfile.write_all(contents.as_bytes()).map_err(io_error)?;
                updated = true;
            } else {
                let mut target_contents = read_file(&dst).map_err(io_error)?;
                if target_contents.contains(&marker) {
                    // we've already got a section, check if it needs updates
                    let mut found_start = false;
                    let mut found_end = false;
                    let mut matches = false;
                    let mut new_lines = Vec::new();
                    let mut output_lines = Vec::new();
                    let marker_line = format!("{} {}", marker, content_hash);
                    output_lines.push(marker_line.clone());
                    output_lines.extend(output.lines().map(|s| s.to_string()));
                    output_lines.push(marker_line.clone());

                    for line in target_contents.lines() {
                        if line.contains(&marker) && !found_start {
                            let old_hash = line.trim_start_matches(&marker).trim();
                            found_start = true;
                            if old_hash == content_hash {
                                // break early, sections match so don't touch the file
                                matches = true;
                                break;
                            } else {
                                // sections don't match, append new section and start ignoring old section
                                new_lines.extend_from_slice(&output_lines);
                            }
                        } else if line.contains(&marker) && found_start {
                            found_end = true;
                        } else if found_start && !found_end {
                            // Ignore these lines, we're between the start and end and we don't match
                        } else {
                            new_lines.push(line.to_string());
                        }
                    }
                    if !matches {
                        let mut outfile = OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .open(&dst)
                            .map_err(io_error)?;
                        outfile
                            .write_all(new_lines.join("\n").as_bytes())
                            .map_err(io_error)?;
                        updated = true;
                    } else {
                        WRITER.write("section matched, skipped");
                        updated = false;
                    }
                } else {
                    // just append, currently doesn't exist
                    target_contents.push_str(&format!(
                        "\n{} {}\n{}\n{} {}\n",
                        marker, content_hash, output, marker, content_hash
                    ));
                    let mut outfile = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&dst)
                        .map_err(io_error)?;
                    outfile
                        .write_all(target_contents.as_bytes())
                        .map_err(io_error)?;
                    updated = true;
                }
            }
            let retval = ctx.create_table()?;
            retval.set("updated", updated)?;
            Ok(retval)
        })?;
        lua_ctx.globals().set("append", f)?;
        Ok(())
    })?;
    Ok(())
}
