use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use mlua::{Lua, Table, UserData};
use reqwest::Url;

use super::{archive::HpgArchive, file::HpgDir};
use crate::Result;
use crate::{
    actions::util,
    error::{self, TaskError},
    WRITER,
};

#[derive(Debug)]
pub enum InstallSource {
    Url { url: Url, archive_path: PathBuf },
    File(PathBuf),
}

#[derive(Debug)]
pub struct HpgInstaller {
    src: InstallSource,
    hash: Option<String>,
    extract_dir: PathBuf,
    install_dir: Option<PathBuf>,
}

impl HpgInstaller {
    fn download(&self) -> Result<PathBuf, mlua::Error> {
        let (u, archive_path) = if let InstallSource::Url { url, archive_path } = &self.src {
            (url, archive_path)
        } else {
            return Err(error::action_error("Called download() on local file"));
        };
        WRITER.write(format!("Downloading {} to {}", u, archive_path.display()));
        let _g = WRITER.enter("installer_download");

        let client = reqwest::blocking::Client::new();
        let builder = client.get(u.clone());
        let mut res = builder
            .send()
            .map_err(|e| error::action_error(format!("{}", e)))?;

        if !res.status().is_success() {
            return Err(error::action_error(format!(
                "Expected 200 status, received {}",
                res.status().as_u16()
            )));
        }
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&archive_path)
            .map_err(error::io_error)?;

        res.copy_to(&mut f)
            .map_err(|e| error::action_error(format!("Body Error: {}", e)))?;

        Ok(archive_path.to_path_buf())
    }

    fn extract(&self, f: HpgArchive) -> Result<HpgDir, mlua::Error> {
        let dir = f.extract(&self.extract_dir)?;
        if let Some(h) = &self.hash {
            let hash_file = self.install_dir().join(".hpg-hash");
            let mut f = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&hash_file)
                .map_err(error::io_error)?;
            f.write_all(h.as_bytes()).map_err(error::io_error)?;
        }
        Ok(dir)
    }

    fn install_dir(&self) -> &Path {
        &self.install_dir.as_ref().unwrap_or(&self.extract_dir)
    }

    fn hash_matches(&self) -> bool {
        if self.install_dir().exists() {
            let hash_file = self.install_dir().join(".hpg-hash");
            let contents = if let Ok(c) = util::read_file(&hash_file) {
                c
            } else {
                return false;
            };
            if let Some(desired) = &self.hash {
                return desired == contents.trim();
            } else {
                return false;
            }
        }
        let archive_path = match &self.src {
            InstallSource::Url {
                url: _,
                archive_path,
            } => archive_path,
            InstallSource::File(f) => f,
        };
        if let Some(desired) = &self.hash {
            if let Ok(h) = crate::hash::file_hash(&archive_path) {
                return desired == &h;
            } else {
                return false;
            }
        } else {
            return false;
        }
    }

    fn install(&self) -> Result<HpgDir, mlua::Error> {
        let f = match &self.src {
            InstallSource::Url { url, archive_path } => {
                let dir = archive_path.parent().ok_or_else(|| {
                    error::action_error(format!("Invalid archive_path {}", &archive_path.display()))
                })?;
                std::fs::create_dir_all(&dir).map_err(error::io_error)?;
                WRITER.write(format!("Installing {}", archive_path.display()));
                let _g = WRITER.enter("installer_install");
                if self.hash_matches() {
                    WRITER.write("Hashes matched, skipped install");
                    return Ok(HpgDir::new(&self.extract_dir));
                }
                let archive = self.download()?;
                if let Some(ty) = HpgArchive::guess_archive_type(url.path()) {
                    HpgArchive::new(archive, ty)
                } else {
                    return Err(error::action_error(format!(
                        "Couldn't guess archive type of url {}",
                        url.path()
                    )));
                }
            }
            InstallSource::File(f) => {
                WRITER.write(format!("Installing {}", f.display()));
                let _g = WRITER.enter("installer_install");
                if self.hash_matches() {
                    WRITER.write("Hashes matched, skipped install");
                    return Ok(HpgDir::new(&self.extract_dir));
                }
                if let Some(ty) = HpgArchive::guess_archive_type(&f.to_string_lossy()) {
                    HpgArchive::new(&f, ty)
                } else {
                    return Err(error::action_error(format!(
                        "Couldn't guess archive type of file {}",
                        &f.display()
                    )));
                }
            }
        };
        self.extract(f)
    }
}

impl UserData for HpgInstaller {
    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("install", |_, this, _: ()| this.install());
        methods.add_method("is_installed", |_, this, _: ()| Ok(this.hash_matches()));
    }
}

pub fn installer(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(
        |_, (archive_path, extract_dir, opts): (String, String, Table)| {
            let url = opts.get::<_, Option<String>>("url")?;
            let hash = opts.get::<_, Option<String>>("hash")?;
            let install_dir = opts.get::<_, Option<String>>("install_dir")?;
            let extract_dir = Path::new(".").join(extract_dir);
            let archive_path = Path::new(".").join(archive_path);

            let install_dir = install_dir.map(|i| Path::new(".").join(i));

            let src = if let Some(u) = url {
                InstallSource::Url {
                    url: Url::parse(&u).map_err(|e| error::action_error(format!("{}", e)))?,
                    archive_path,
                }
            } else {
                InstallSource::File(archive_path)
            };

            let i = HpgInstaller {
                src,
                hash,
                extract_dir,
                install_dir,
            };
            i.install()
        },
    )?;
    lua.globals().set("install", f)?;
    Ok(())
}
