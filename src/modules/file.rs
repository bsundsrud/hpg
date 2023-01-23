use std::{
    fs::{File, OpenOptions, Permissions},
    io::Write,
    os::unix::{fs::symlink, prelude::PermissionsExt},
    path::{Path, PathBuf},
};

use nix::unistd::{geteuid, User};
use rlua::{Function, Lua, MetaMethod, Table, UserData, Value};

use crate::{actions::util, error::TaskError, hash, Result, WRITER};

pub struct HpgFile {
    path: PathBuf,
}

pub struct HpgDir {
    path: PathBuf,
}

impl HpgFile {
    pub fn new<P: Into<PathBuf>>(path: P) -> HpgFile {
        HpgFile { path: path.into() }
    }
}

impl UserData for HpgFile {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(this.path.to_string_lossy().to_string())
        });

        methods.add_method("contents", |_, this, _: ()| {
            WRITER.write(format!("file_contents {}", &this.path.to_string_lossy()));
            let _g = WRITER.enter("file_contents");

            let contents = util::read_file(&this.path).map_err(util::io_error)?;
            Ok(contents)
        });
        methods.add_method("exists", |_, this, _: ()| {
            let exists = this.path.exists();

            Ok(exists)
        });
        methods.add_method("hash", |_, this, _: ()| {
            hash::file_hash(&this.path).map_err(util::io_error)
        });
        methods.add_method("chown", |_, this, opts: Table| {
            let user: Option<rlua::Value> = opts.get("user")?;
            let group: Option<rlua::Value> = opts.get("group")?;
            WRITER.write(format!("Chown {}:", &this.path.to_string_lossy()));
            let _g = WRITER.enter("chown");

            util::run_chown(&this.path, user, group)?;
            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("chmod", |_, this, mode: String| {
            let mode = u32::from_str_radix(&mode, 8)
                .map_err(|e| util::action_error(format!("Invalid Mode {}: {}", mode, e)))?;
            WRITER.write(format!("Chmod {} {}", &this.path.to_string_lossy(), mode));
            let _g = WRITER.enter("chmod");

            let f = File::open(&this.path).map_err(util::io_error)?;
            f.set_permissions(Permissions::from_mode(mode))
                .map_err(util::io_error)?;

            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("copy", |_, this, dst: String| {
            let src_contents = util::read_file(&this.path).map_err(util::io_error)?;
            let cwd = Path::new(".");
            let dst = cwd.join(&dst);

            WRITER.write(format!(
                "copy {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            ));

            let _g = WRITER.enter("copy");
            let updated: bool;
            if should_update_file(&dst, &src_contents).map_err(util::io_error)? {
                let mut outfile = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&dst)
                    .map_err(util::io_error)?;
                outfile
                    .write_all(src_contents.as_bytes())
                    .map_err(util::io_error)?;
                updated = true;
            } else {
                WRITER.write("files matched, skipped");
                updated = false;
            }
            Ok(updated)
        });
        methods.add_method(
            "template",
            |ctx, this, (dst, template_context): (String, Option<Table>)| {
                let cwd = Path::new(".");
                let dst = cwd.join(&dst);
                WRITER.write(format!(
                    "render template {} to {}",
                    &this.path.to_string_lossy(),
                    &dst.to_string_lossy()
                ));
                let _g = WRITER.enter("template");

                let template_context = if let Some(c) = template_context {
                    c
                } else {
                    ctx.create_table()?
                };
                let template_context = util::lua_table_to_json(template_context)
                    .map_err(|e| util::action_error(format!("Unable to parse context: {}", e)))?;

                let src_contents = run_template_file(&this.path, template_context)
                    .map_err(|e| util::task_error(e))?;
                let updated: bool;
                if should_update_file(&dst, &src_contents).map_err(util::io_error)? {
                    let mut outfile = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&dst)
                        .map_err(util::io_error)?;
                    outfile
                        .write_all(src_contents.as_bytes())
                        .map_err(util::io_error)?;
                    updated = true;
                } else {
                    WRITER.write("files matched, skipped");
                    updated = false;
                }
                Ok(updated)
            },
        );
        methods.add_method("symlink", |_, this, dst: String| {
            let cwd = Path::new(".");
            let dst = cwd.join(&dst);
            WRITER.write(format!(
                "Symlink {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            ));
            let _g = WRITER.enter("symlink_file");
            if dst.exists() {
                std::fs::remove_file(&dst).map_err(util::io_error)?;
            }
            symlink(&this.path, &dst).map_err(util::io_error)?;
            Ok(HpgFile::new(dst))
        });

        methods.add_method("touch", |_, this, _: ()| {
            WRITER.write(format!("touch {}", &this.path.to_string_lossy()));
            let _g = WRITER.enter("touch");
            let f = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&this.path)
                .map_err(util::io_error)?;
            drop(f);
            Ok(HpgFile::new(&this.path))
        });

        methods.add_method("append", |_, this, opts: Table| {
            WRITER.write(format!("append to {}", &this.path.to_string_lossy()));
            let _g = WRITER.enter("append");

            let src = opts.get::<_, Option<String>>("src")?;
            let contents = opts.get::<_, Option<String>>("contents")?;
            let input = match (src, contents) {
                (None, None) => {
                    return Err(util::action_error(
                        "append: Must specify one of 'src' or 'contents'",
                    ))
                }
                (None, Some(s)) => s,
                (Some(path), None) => util::read_file(Path::new(&path)).map_err(util::io_error)?,
                (Some(_), Some(_)) => {
                    return Err(util::action_error(
                        "append: Must specify only one of 'src' or 'contents'",
                    ))
                }
            };
            let marker = opts
                .get::<_, Option<String>>("marker")?
                .ok_or_else(|| util::action_error("append: 'marker' is required"))?;
            let content_hash = hash::content_hash(&input);
            let updated = append_to_existing(&this.path, &marker, &input, &content_hash)?;
            Ok(updated)
        });

        methods.add_method("append_template", |ctx, this, opts: Table| {
            let src = opts.get::<_, Option<String>>("src")?;
            let contents = opts.get::<_, Option<String>>("contents")?;
            WRITER.write(format!(
                "append template to {}",
                &this.path.to_string_lossy()
            ));
            let _g = WRITER.enter("append_template");

            let input = match (src, contents) {
                (None, None) => {
                    return Err(util::action_error(
                        "append: Must specify one of 'src' or 'contents'",
                    ))
                }
                (None, Some(s)) => s,
                (Some(path), None) => util::read_file(Path::new(&path)).map_err(util::io_error)?,
                (Some(_), Some(_)) => {
                    return Err(util::action_error(
                        "append: Must specify only one of 'src' or 'contents'",
                    ))
                }
            };
            let marker = opts
                .get::<_, Option<String>>("marker")?
                .ok_or_else(|| util::action_error("append: 'marker' is required"))?;
            let template_context = opts.get::<_, Option<Table>>("context")?;
            let template_context = if let Some(c) = template_context {
                c
            } else {
                ctx.create_table()?
            };
            let template_context = util::lua_table_to_json(template_context)
                .map_err(|e| util::action_error(format!("Unable to parse context: {}", e)))?;
            let input = run_template(&input, template_context)
                .map_err(|e| util::action_error(e.to_string()))?;
            let content_hash = hash::content_hash(&input);
            let updated = append_to_existing(&this.path, &marker, &input, &content_hash)?;
            Ok(updated)
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(this.path.to_string_lossy().to_string())
        });
        methods.add_meta_method(MetaMethod::Concat, |ctx, this, other: Value| {
            let globals = ctx.globals();
            let tostring: Function = globals.get("tostring")?;
            let s = tostring.call::<_, String>(other)?;
            let mut joined = this.path.to_string_lossy().to_string();
            joined.push_str(&s);
            Ok(joined)
        });
    }
}

impl HpgDir {
    pub fn new<P: Into<PathBuf>>(path: P) -> HpgDir {
        HpgDir { path: path.into() }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl UserData for HpgDir {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("exists", |_, this, _: ()| {
            let exists = this.path.exists();

            Ok(exists)
        });

        methods.add_method("chown", |_, this, opts: Table| {
            let user: Option<rlua::Value> = opts.get("user")?;
            let group: Option<rlua::Value> = opts.get("group")?;
            WRITER.write(format!("Chown {}:", &this.path.to_string_lossy()));
            let _g = WRITER.enter("chown");

            util::run_chown(&this.path, user, group)?;
            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("chmod", |_, this, mode_str: String| {
            let mode = u32::from_str_radix(&mode_str, 8)
                .map_err(|e| util::action_error(format!("Invalid Mode {}: {}", mode_str, e)))?;
            WRITER.write(format!(
                "Chmod {} {}",
                &this.path.to_string_lossy(),
                mode_str
            ));
            let _g = WRITER.enter("chmod");

            let f = File::open(&this.path).map_err(util::io_error)?;
            f.set_permissions(Permissions::from_mode(mode))
                .map_err(util::io_error)?;

            Ok(HpgFile::new(&this.path))
        });

        methods.add_method("mkdir", |_, this, _: ()| {
            WRITER.write(format!("mkdir {}", &this.path.to_string_lossy()));
            WRITER.enter("mkdir");

            std::fs::create_dir_all(&this.path).map_err(util::io_error)?;
            Ok(HpgDir::new(&this.path))
        });

        methods.add_method("symlink", |_, this, dst: String| {
            let cwd = Path::new(".");
            let dst = cwd.join(&dst);
            WRITER.write(format!(
                "Symlink {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            ));
            WRITER.enter("symlink_dir");
            if dst.exists() {
                std::fs::remove_file(&dst).map_err(util::io_error)?;
            }
            symlink(&this.path, &dst).map_err(util::io_error)?;
            Ok(HpgFile::new(dst))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(this.path.to_string_lossy().to_string())
        });
        methods.add_meta_method(MetaMethod::Concat, |ctx, this, other: Value| {
            let globals = ctx.globals();
            let tostring: Function = globals.get("tostring")?;
            let s = tostring.call::<_, String>(other)?;
            let mut joined = this.path.to_string_lossy().to_string();
            joined.push_str(&s);
            Ok(joined)
        });
    }
}

pub fn file(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, path: String| {
            let cwd = Path::new(".");
            let p = cwd.join(&path);
            if p.exists() && !p.is_file() {
                return Err(util::action_error(format!(
                    "Path {} already exists and is not a file",
                    &p.to_string_lossy()
                )));
            }
            Ok(HpgFile::new(&p))
        })?;
        lua_ctx.globals().set("file", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn dir(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, path: String| {
            let cwd = Path::new(".");
            let p = cwd.join(&path);
            if p.exists() && p.is_file() {
                return Err(util::action_error(format!(
                    "Path {} already exists and is a file",
                    &p.to_string_lossy()
                )));
            }
            Ok(HpgDir::new(&p))
        })?;
        lua_ctx.globals().set("dir", f)?;
        Ok(())
    })?;
    Ok(())
}

pub fn homedir(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let f = lua_ctx.create_function(|_, user: Option<String>| {
            let p = if let Some(username) = user {
                let u = User::from_name(&username)
                    .map_err(|e| {
                        util::action_error(format!("user syscall for {}: {}", &username, e))
                    })?
                    .ok_or_else(|| util::action_error(format!("Unknown user {}", &username)))?;
                u.dir.clone()
            } else {
                let euid = geteuid();
                let u = User::from_uid(euid)
                    .map_err(|e| util::action_error(format!("uid syscall for {}: {}", &euid, e)))?
                    .ok_or_else(|| util::action_error(format!("Unknown uid {}", &euid)))?;
                u.dir.clone()
            };
            if p.exists() && p.is_file() {
                return Err(util::action_error(format!(
                    "Path {} already exists and is a file",
                    &p.to_string_lossy()
                )));
            }
            Ok(HpgDir::new(&p))
        })?;
        lua_ctx.globals().set("homedir", f)?;
        Ok(())
    })?;
    Ok(())
}

fn append_to_existing(
    dst: &Path,
    marker: &str,
    content: &str,
    hash: &str,
) -> Result<bool, rlua::Error> {
    let updated;
    if !dst.exists() {
        let contents = format!("{} {}\n{}\n{} {}\n", marker, hash, content, marker, hash);
        let mut outfile = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&dst)
            .map_err(util::io_error)?;
        outfile
            .write_all(contents.as_bytes())
            .map_err(util::io_error)?;
        updated = true;
    } else {
        let mut target_contents = util::read_file(&dst).map_err(util::io_error)?;
        if target_contents.contains(&marker) {
            // we've already got a section, check if it needs updates
            let mut found_start = false;
            let mut found_end = false;
            let mut matches = false;
            let mut new_lines = Vec::new();
            let mut output_lines = Vec::new();
            let marker_line = format!("{} {}", marker, hash);
            output_lines.push(marker_line.clone());
            output_lines.extend(content.lines().map(|s| s.to_string()));
            output_lines.push(marker_line.clone());

            for line in target_contents.lines() {
                if line.contains(&marker) && !found_start {
                    let old_hash = line.trim_start_matches(&marker).trim();
                    found_start = true;
                    if old_hash == hash {
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
                    .map_err(util::io_error)?;
                outfile
                    .write_all(new_lines.join("\n").as_bytes())
                    .map_err(util::io_error)?;
                updated = true;
            } else {
                WRITER.write("section matched, skipped");
                updated = false;
            }
        } else {
            // just append, currently doesn't exist
            target_contents.push_str(&format!(
                "\n{} {}\n{}\n{} {}\n",
                marker, hash, content, marker, hash
            ));
            let mut outfile = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&dst)
                .map_err(util::io_error)?;
            outfile
                .write_all(target_contents.as_bytes())
                .map_err(util::io_error)?;
            updated = true;
        }
    }
    Ok(updated)
}

fn should_update_file(dst: &Path, contents: &str) -> Result<bool, std::io::Error> {
    if !dst.exists() || !dst.is_file() {
        Ok(true)
    } else {
        Ok(hash::file_hash(&dst)? != hash::content_hash(&contents))
    }
}

fn run_template_file(tmpl_path: &Path, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::ActionError(format!("Invalid context: {}", e)))?;
    let tmpl_contents = util::read_file(tmpl_path)?;
    let rendered = tera::Tera::one_off(&tmpl_contents, &ctx, false)
        .map_err(|e| TaskError::TemplateError(e))?;
    Ok(rendered)
}

fn run_template(tmpl: &str, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::ActionError(format!("Invalid context: {}", e)))?;
    let rendered =
        tera::Tera::one_off(&tmpl, &ctx, false).map_err(|e| TaskError::TemplateError(e))?;
    Ok(rendered)
}
