use std::{
    fs::{File, OpenOptions, Permissions},
    io::Write,
    os::unix::{fs::symlink, prelude::PermissionsExt},
    path::{Path, PathBuf},
};

use mlua::{Function, Lua, MetaMethod, Table, UserData, Value};
use nix::unistd::{geteuid, Uid, User};

use crate::{
    actions::util,
    error::{self, TaskError},
    hash, indent_output, output, Result,
};

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
    fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("path", |_, this| {
            Ok(this.path.to_string_lossy().to_string())
        });

        fields.add_field_method_get("canonical_path", |_, this| {
            Ok(this.path.canonicalize()?.to_string_lossy().to_string())
        });
    }

    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(this.path.to_string_lossy().to_string())
        });

        methods.add_method("contents", |_, this, _: ()| {
            output!("file_contents {}", &this.path.to_string_lossy());

            let contents = util::read_file(&this.path).map_err(error::io_error)?;
            Ok(contents)
        });
        methods.add_method("exists", |_, this, _: ()| {
            let exists = this.path.exists();

            Ok(exists)
        });
        methods.add_method("hash", |_, this, _: ()| {
            hash::file_hash(&this.path).map_err(error::io_error)
        });
        methods.add_method("chown", |_, this, opts: Table| {
            let user: Option<mlua::Value> = opts.get("user")?;
            let group: Option<mlua::Value> = opts.get("group")?;
            output!("Chown {}:", &this.path.to_string_lossy());
            let uid = user
                .map(|u| util::uid_for_value(&u))
                .map_or(Ok(None), |v| v.map(Some))?; // Flip Option<Result<_, _>> to Result<Option<_>, _>
            let gid = group
                .map(|u| util::gid_for_value(&u))
                .map_or(Ok(None), |v| v.map(Some))?; // Flip Option<Result<_, _>> to Result<Option<_>, _>

            util::run_chown(&this.path, uid, gid)?;
            if let Some(uid) = uid {
                indent_output!(1, "uid: {}", uid);
            }
            if let Some(gid) = gid {
                indent_output!(1, "gid: {}", gid);
            }

            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("chmod", |_, this, mode: String| {
            let mode = u32::from_str_radix(&mode, 8)
                .map_err(|e| error::action_error(format!("Invalid Mode {}: {}", mode, e)))?;
            output!("Chmod {} {}", &this.path.to_string_lossy(), mode);

            let f = File::open(&this.path).map_err(error::io_error)?;
            f.set_permissions(Permissions::from_mode(mode))
                .map_err(error::io_error)?;

            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("copy", |_, this, dst: String| {
            let src_contents = util::read_file(&this.path).map_err(error::io_error)?;
            let cwd = Path::new(".");
            let dst = cwd.join(dst);
            let dst = if dst.is_dir() {
                dst.join(this.path.file_name().unwrap())
            } else {
                dst
            };
            output!(
                "copy {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            );

            let updated = if should_update_file(&dst, &src_contents).map_err(error::io_error)? {
                let mut outfile = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&dst)
                    .map_err(error::io_error)?;
                outfile.write_all(&src_contents).map_err(error::io_error)?;
                true
            } else {
                indent_output!(1, "files matched, skipped");
                false
            };
            Ok(updated)
        });
        methods.add_method(
            "template",
            |ctx, this, (dst, template_context): (String, Option<Table>)| {
                let cwd = Path::new(".");
                let dst = cwd.join(dst);
                output!(
                    "render template {} to {}",
                    &this.path.to_string_lossy(),
                    &dst.to_string_lossy()
                );
                let template_context = if let Some(c) = template_context {
                    c
                } else {
                    ctx.create_table()?
                };
                let template_context = util::lua_table_to_json(template_context)
                    .map_err(|e| error::action_error(format!("Unable to parse context: {}", e)))?;

                let src_contents =
                    run_template_file(&this.path, template_context).map_err(error::task_error)?;

                let updated = if should_update_file(&dst, &src_contents.as_bytes())
                    .map_err(error::io_error)?
                {
                    let mut outfile = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&dst)
                        .map_err(error::io_error)?;
                    outfile
                        .write_all(src_contents.as_bytes())
                        .map_err(error::io_error)?;
                    true
                } else {
                    indent_output!(1, "files matched, skipped");
                    false
                };
                Ok(updated)
            },
        );
        methods.add_method("symlink", |_, this, dst: String| {
            let cwd = Path::new(".");
            let dst = cwd.join(dst);
            output!(
                "Symlink {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            );
            if dst.exists() {
                std::fs::remove_file(&dst).map_err(error::io_error)?;
            }
            symlink(&this.path, &dst).map_err(error::io_error)?;
            Ok(HpgFile::new(dst))
        });

        methods.add_method("touch", |_, this, _: ()| {
            output!("touch {}", &this.path.to_string_lossy());
            let f = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&this.path)
                .map_err(error::io_error)?;
            drop(f);
            Ok(HpgFile::new(&this.path))
        });

        methods.add_method("append", |_, this, opts: Table| {
            output!("append to {}", &this.path.to_string_lossy());
            let src = opts.get::<_, Option<String>>("src")?;
            let contents = opts.get::<_, Option<String>>("contents")?;
            let input = match (src, contents) {
                (None, None) => {
                    return Err(error::action_error(
                        "append: Must specify one of 'src' or 'contents'",
                    ))
                }
                (None, Some(s)) => s,
                (Some(path), None) => String::from_utf8_lossy(
                    &util::read_file(Path::new(&path)).map_err(error::io_error)?,
                )
                .to_string(),
                (Some(_), Some(_)) => {
                    return Err(error::action_error(
                        "append: Must specify only one of 'src' or 'contents'",
                    ))
                }
            };
            let marker = opts
                .get::<_, Option<String>>("marker")?
                .ok_or_else(|| error::action_error("append: 'marker' is required"))?;
            let content_hash = hash::content_hash(&input.as_bytes());
            let updated = append_to_existing(&this.path, &marker, &input, &content_hash)?;
            Ok(updated)
        });

        methods.add_method("append_template", |ctx, this, opts: Table| {
            let src = opts.get::<_, Option<String>>("src")?;
            let contents = opts.get::<_, Option<String>>("contents")?;
            output!("append template to {}", &this.path.to_string_lossy());

            let input = match (src, contents) {
                (None, None) => {
                    return Err(error::action_error(
                        "append: Must specify one of 'src' or 'contents'",
                    ))
                }
                (None, Some(s)) => s,
                (Some(path), None) => String::from_utf8_lossy(
                    &util::read_file(Path::new(&path)).map_err(error::io_error)?,
                )
                .to_string(),
                (Some(_), Some(_)) => {
                    return Err(error::action_error(
                        "append: Must specify only one of 'src' or 'contents'",
                    ))
                }
            };
            let marker = opts
                .get::<_, Option<String>>("marker")?
                .ok_or_else(|| error::action_error("append: 'marker' is required"))?;
            let template_context = opts.get::<_, Option<Table>>("context")?;
            let template_context = if let Some(c) = template_context {
                c
            } else {
                ctx.create_table()?
            };
            let template_context = util::lua_table_to_json(template_context)
                .map_err(|e| error::action_error(format!("Unable to parse context: {}", e)))?;
            let input = run_template(&input, template_context)
                .map_err(|e| error::action_error(e.to_string()))?;
            let content_hash = hash::content_hash(&input.as_bytes());
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
}

impl UserData for HpgDir {
    fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("path", |_, this| {
            Ok(this.path.to_string_lossy().to_string())
        });

        fields.add_field_method_get("canonical_path", |_, this| {
            Ok(this.path.canonicalize()?.to_string_lossy().to_string())
        });
    }

    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("exists", |_, this, _: ()| {
            let exists = this.path.exists();

            Ok(exists)
        });

        methods.add_method("chown", |_, this, opts: Table| {
            let user: Option<mlua::Value> = opts.get("user")?;
            let group: Option<mlua::Value> = opts.get("group")?;
            let recursive: Option<bool> = opts.get("recursive")?;
            let recursive = recursive.unwrap_or(false);
            let uid = user
                .map(|u| util::uid_for_value(&u))
                .map_or(Ok(None), |v| v.map(Some))?; // Flip Option<Result<_, _>> to Result<Option<_>, _>
            let gid = group
                .map(|u| util::gid_for_value(&u))
                .map_or(Ok(None), |v| v.map(Some))?; // Flip Option<Result<_, _>> to Result<Option<_>, _>

            if recursive {
                output!("Chown {} (recursive):", &this.path.to_string_lossy());
                util::run_chown_recursive(&this.path, uid, gid)?;
            } else {
                output!("Chown {}:", &this.path.to_string_lossy());
                util::run_chown(&this.path, uid, gid)?;
            }
            if let Some(uid) = uid {
                indent_output!(1, "uid: {}", uid);
            }
            if let Some(gid) = gid {
                indent_output!(1, "gid: {}", gid);
            }
            Ok(HpgFile::new(&this.path))
        });
        methods.add_method("chmod", |_, this, mode_str: String| {
            let mode = u32::from_str_radix(&mode_str, 8)
                .map_err(|e| error::action_error(format!("Invalid Mode {}: {}", mode_str, e)))?;
            output!("Chmod {} {}", &this.path.to_string_lossy(), mode_str);
            let f = File::open(&this.path).map_err(error::io_error)?;
            f.set_permissions(Permissions::from_mode(mode))
                .map_err(error::io_error)?;

            Ok(HpgFile::new(&this.path))
        });

        methods.add_method("mkdir", |_, this, _: ()| {
            output!("mkdir {}", &this.path.to_string_lossy());

            std::fs::create_dir_all(&this.path).map_err(error::io_error)?;
            Ok(HpgDir::new(&this.path))
        });

        methods.add_method("symlink", |_, this, dst: String| {
            let cwd = Path::new(".");
            let dst = cwd.join(dst);
            output!(
                "Symlink {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            );
            if dst.exists() {
                std::fs::remove_file(&dst).map_err(error::io_error)?;
            }
            symlink(&this.path, &dst).map_err(error::io_error)?;
            Ok(HpgFile::new(dst))
        });

        methods.add_method("copy", |_, this, dst: String| {
            output!("Copy directory {} to {}", this.path.to_string_lossy(), dst);
            let last_segment = this.path.file_name().unwrap();
            let dst_path = PathBuf::from(&dst).join(last_segment);
            copy_dir_all(&this.path, &dst_path)?;

            Ok(HpgDir::new(dst_path))
        });

        methods.add_method("copy_contents", |_, this, dst: String| {
            output!(
                "Copy directory contents from {} to {}",
                this.path.to_string_lossy(),
                dst
            );
            copy_dir_all(&this.path, &dst)?;

            Ok(HpgDir::new(dst))
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

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            let src_contents = util::read_file(&entry.path())?;
            let dst_file = dst.as_ref().join(entry.file_name());
            if should_update_file(&dst_file, &src_contents)? {
                std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
                indent_output!(1, "Updating file {}", dst_file.to_string_lossy());
            } else {
                indent_output!(1, "{} is up-to-date.", dst_file.to_string_lossy());
            }
        }
    }
    Ok(())
}

pub fn file(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, path: String| {
        let cwd = Path::new(".");
        let p = cwd.join(path);
        if p.exists() && !p.is_file() {
            return Err(error::action_error(format!(
                "Path {} already exists and is not a file",
                &p.to_string_lossy()
            )));
        }
        Ok(HpgFile::new(&p))
    })?;
    lua.globals().set("file", f)?;
    Ok(())
}

pub fn dir(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, path: String| {
        let cwd = Path::new(".");
        let p = cwd.join(path);
        if p.exists() && p.is_file() {
            return Err(error::action_error(format!(
                "Path {} already exists and is a file",
                &p.to_string_lossy()
            )));
        }
        Ok(HpgDir::new(&p))
    })?;
    lua.globals().set("dir", f)?;
    Ok(())
}

pub fn homedir(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|_, user: Option<String>| {
        let p = if let Some(username) = user {
            let u = User::from_name(&username)
                .map_err(|e| error::action_error(format!("user syscall for {}: {}", &username, e)))?
                .ok_or_else(|| error::action_error(format!("Unknown user {}", &username)))?;
            u.dir.clone()
        } else {
            let euid = geteuid();
            let u = User::from_uid(euid)
                .map_err(|e| error::action_error(format!("uid syscall for {}: {}", &euid, e)))?
                .ok_or_else(|| error::action_error(format!("Unknown uid {}", &euid)))?;
            u.dir.clone()
        };
        if p.exists() && p.is_file() {
            return Err(error::action_error(format!(
                "Path {} already exists and is a file",
                &p.to_string_lossy()
            )));
        }
        Ok(HpgDir::new(&p))
    })?;
    lua.globals().set("homedir", f)?;
    Ok(())
}

fn append_to_existing(
    dst: &Path,
    marker: &str,
    content: &str,
    hash: &str,
) -> Result<bool, mlua::Error> {
    let updated;
    if !dst.exists() {
        let contents = format!("{} {}\n{}\n{} {}\n", marker, hash, content, marker, hash);
        let mut outfile = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dst)
            .map_err(error::io_error)?;
        outfile
            .write_all(contents.as_bytes())
            .map_err(error::io_error)?;
        updated = true;
    } else {
        let mut target_contents =
            String::from_utf8_lossy(&util::read_file(dst).map_err(error::io_error)?).to_string();
        if target_contents.contains(marker) {
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
                if line.contains(marker) && !found_start {
                    let old_hash = line.trim_start_matches(marker).trim();
                    found_start = true;
                    if old_hash == hash {
                        // break early, sections match so don't touch the file
                        matches = true;
                        break;
                    } else {
                        // sections don't match, append new section and start ignoring old section
                        new_lines.extend_from_slice(&output_lines);
                    }
                } else if line.contains(marker) && found_start {
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
                    .open(dst)
                    .map_err(error::io_error)?;
                outfile
                    .write_all(new_lines.join("\n").as_bytes())
                    .map_err(error::io_error)?;
                updated = true;
            } else {
                indent_output!(1, "section matched, skipped");
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
                .open(dst)
                .map_err(error::io_error)?;
            outfile
                .write_all(target_contents.as_bytes())
                .map_err(error::io_error)?;
            updated = true;
        }
    }
    Ok(updated)
}

fn should_update_file(dst: &Path, contents: &[u8]) -> Result<bool, std::io::Error> {
    if !dst.exists() || !dst.is_file() {
        Ok(true)
    } else {
        Ok(hash::file_hash(dst)? != hash::content_hash(contents))
    }
}

fn run_template_file(tmpl_path: &Path, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::Action(format!("Invalid context: {}", e)))?;
    let tmpl_contents = String::from_utf8_lossy(&util::read_file(tmpl_path)?).to_string();
    let rendered = tera::Tera::one_off(&tmpl_contents, &ctx, false).map_err(TaskError::Template)?;
    Ok(rendered)
}

fn run_template(tmpl: &str, context: serde_json::Value) -> Result<String, TaskError> {
    let ctx = tera::Context::from_value(context)
        .map_err(|e| TaskError::Action(format!("Invalid context: {}", e)))?;
    let rendered = tera::Tera::one_off(tmpl, &ctx, false).map_err(TaskError::Template)?;
    Ok(rendered)
}
