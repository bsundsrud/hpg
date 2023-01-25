use flate2::read::GzDecoder;
use std::fmt::Display;
use std::fs::File;
use std::path::{Path, PathBuf};
use tar::Archive;
use zip::ZipArchive;

use mlua::{Lua, Table, UserData};

use crate::error::{self, TaskError};
use crate::{Result, WRITER};

use super::file::HpgDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    Gzip,
    Bzip2,
}

impl Display for CompressionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionType::Gzip => write!(f, "gzip"),
            CompressionType::Bzip2 => write!(f, "bz2"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ArchiveType {
    Zip,
    Tarball(Option<CompressionType>),
}

impl ArchiveType {
    pub fn zip() -> ArchiveType {
        ArchiveType::Zip
    }

    pub fn gzip_tarball() -> ArchiveType {
        ArchiveType::Tarball(Some(CompressionType::Gzip))
    }

    pub fn bzip2_tarball() -> ArchiveType {
        ArchiveType::Tarball(Some(CompressionType::Bzip2))
    }

    pub fn plain_tarball() -> ArchiveType {
        ArchiveType::Tarball(None)
    }
}

impl Display for ArchiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveType::Zip => write!(f, "zip"),
            ArchiveType::Tarball(Some(c)) => write!(f, "tar/{}", c),
            ArchiveType::Tarball(None) => write!(f, "tar"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HpgArchive {
    path: PathBuf,
    ty: ArchiveType,
}

impl HpgArchive {
    pub fn new<P: Into<PathBuf>>(path: P, ty: ArchiveType) -> HpgArchive {
        HpgArchive {
            path: path.into(),
            ty,
        }
    }

    pub fn extract(&self, dst: &Path) -> Result<HpgDir, mlua::Error> {
        match self.ty {
            ArchiveType::Zip => extract_zip(&self.path, &dst)?,
            ArchiveType::Tarball(ty) => extract_tarball(&self.path, &dst, &ty)?,
        }
        Ok(HpgDir::new(dst))
    }

    pub fn guess_archive_type(f: &str) -> Option<ArchiveType> {
        if f.ends_with(".tar.gz") || f.ends_with(".tgz") {
            Some(ArchiveType::Tarball(Some(CompressionType::Gzip)))
        } else if f.ends_with(".tar.bz2") {
            Some(ArchiveType::Tarball(Some(CompressionType::Bzip2)))
        } else if f.ends_with(".tar") {
            Some(ArchiveType::Tarball(None))
        } else if f.ends_with(".zip") {
            Some(ArchiveType::Zip)
        } else {
            None
        }
    }
}

impl UserData for HpgArchive {
    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("extract", |_ctx, this, dst: String| {
            let dst = Path::new(".").join(&dst);
            WRITER.write(format!(
                "Extract {} to {}",
                &this.path.to_string_lossy(),
                &dst.to_string_lossy()
            ));
            let _ = WRITER.enter("archive_extract");
            this.extract(&dst)
        });
    }
}

fn extract_zip(src: &Path, dst: &Path) -> Result<(), mlua::Error> {
    let f = File::open(&src).map_err(error::io_error)?;
    let mut archive =
        ZipArchive::new(&f).map_err(|e| error::action_error(format!("Zip Error: {}", e)))?;
    archive
        .extract(&dst)
        .map_err(|e| error::action_error(format!("Zip Error: {}", e)))?;
    Ok(())
}

fn extract_tarball(
    src: &Path,
    dst: &Path,
    ty: &Option<CompressionType>,
) -> Result<(), mlua::Error> {
    let f = File::open(&src).map_err(error::io_error)?;
    match *ty {
        Some(CompressionType::Gzip) => {
            let tar = GzDecoder::new(f);
            let mut archive = Archive::new(tar);
            archive.unpack(&dst).map_err(error::io_error)?;
        }
        Some(CompressionType::Bzip2) => unimplemented!(),
        None => {
            let mut archive = Archive::new(f);
            archive.unpack(&dst).map_err(error::io_error)?;
        }
    }

    Ok(())
}

pub fn archive(lua: &Lua) -> Result<(), TaskError> {
    let f = lua.create_function(|ctx, (path, opts): (String, Option<Table>)| {
        let opts = if let Some(o) = opts {
            o
        } else {
            ctx.create_table()?
        };
        let ty = opts.get::<_, Option<String>>("type")?;
        let ty_ref = ty.as_ref().map(|s| s.as_str());
        let compression = opts.get::<_, Option<String>>("compression")?;
        let comp_ref = compression.as_ref().map(|s| s.as_str());
        let src = Path::new(".").join(&path);
        let archive_ty = match (ty_ref, comp_ref) {
            (Some("zip"), _) => ArchiveType::zip(),
            (Some("tar"), Some("gz")) => ArchiveType::gzip_tarball(),
            (Some("tar"), Some("bz2")) => ArchiveType::bzip2_tarball(),
            (Some("tar"), None) => ArchiveType::plain_tarball(),
            (None, None) => {
                HpgArchive::guess_archive_type(&src.to_string_lossy()).ok_or_else(|| {
                    error::action_error(format!(
                        "Couldn't guess the archive type of {}",
                        &src.to_string_lossy()
                    ))
                })?
            }
            _ => return Err(error::action_error("Unknown type/compression combination")),
        };

        Ok(HpgArchive::new(&path, archive_ty))
    })?;

    lua.globals().set("archive", f)?;
    Ok(())
}
