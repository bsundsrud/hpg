use std::path::Path;

use ignore::{
    overrides::OverrideBuilder,
    WalkBuilder,
};
use pathdiff::diff_paths;

use crate::error::HpgRemoteError;

use super::messages::{FileType, LocalFile};

pub fn find_hpg_files(root: &Path) -> Result<Vec<LocalFile>, HpgRemoteError> {
    let mut files = Vec::new();
    // Overrides work the opposite here.  Lines with ! ignore, otherwise
    // it's treated as a whitelist
    let overrides = OverrideBuilder::new(root)
        .case_insensitive(true)?
        .add("!.meta/")?
        .add("!.hpgignore")?
        .add("!inventory.yaml")?
        .add("!inventory.yml")?
        .add("!inventory.json")?
        .build()?;
    for res in WalkBuilder::new(root)
        .add_custom_ignore_filename(".hpgignore")
        .overrides(overrides)
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
