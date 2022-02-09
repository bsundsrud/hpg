use flate2::read::GzDecoder;
use std::fs::File;
use std::path::{Path, PathBuf};
use tar::Archive;
use zip::ZipArchive;

use rlua::UserData;

use crate::actions::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    Gzip,
    Bzip2,
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
}

impl UserData for HpgArchive {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("extract", |_ctx, this, dst: String| {
            let dst = Path::new(".").join(&dst);
            match this.ty {
                ArchiveType::Zip => extract_zip(&this.path, &dst)?,
                ArchiveType::Tarball(ty) => extract_tarball(&this.path, &dst, &ty)?,
            }
            Ok(())
        });
    }
}

fn extract_zip(src: &Path, dst: &Path) -> Result<(), rlua::Error> {
    let f = File::open(&src).map_err(util::io_error)?;
    let mut archive =
        ZipArchive::new(&f).map_err(|e| util::action_error(format!("Zip Error: {}", e)))?;
    archive
        .extract(&dst)
        .map_err(|e| util::action_error(format!("Zip Error: {}", e)))?;
    Ok(())
}

fn extract_tarball(
    src: &Path,
    dst: &Path,
    ty: &Option<CompressionType>,
) -> Result<(), rlua::Error> {
    let f = File::open(&src).map_err(util::io_error)?;
    match *ty {
        Some(CompressionType::Gzip) => {
            let tar = GzDecoder::new(f);
            let mut archive = Archive::new(tar);
            archive.unpack(&dst).map_err(util::io_error)?;
        }
        Some(CompressionType::Bzip2) => unimplemented!(),
        None => {
            let mut archive = Archive::new(f);
            archive.unpack(&dst).map_err(util::io_error)?;
        }
    }

    Ok(())
}
