use std::path::Path;

use ignore::WalkBuilder;
use pathdiff::diff_paths;

use crate::{
    error::Result,
    types::{FileType, LocalFile},
};

pub fn find_hpg_files(root: &Path) -> Result<Vec<LocalFile>> {
    let mut files = Vec::new();

    for res in WalkBuilder::new(root)
        .add_custom_ignore_filename(".hpgignore")
        .build()
    {
        let f = res?;
        let ty = if f.path().is_dir() {
            FileType::Dir
        } else if f.path().is_file() {
            FileType::File
        } else {
            unreachable!()
        };
        files.push(LocalFile {
            ty,
            rel_path: diff_paths(f.path(), root).unwrap(),
        });
    }
    Ok(files)
}
