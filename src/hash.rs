use sha2::{Digest, Sha256};
use std::{fs::File, io::Error as IoError, path::Path};

pub fn file_hash(path: &Path) -> Result<String, IoError> {
    let mut f = File::open(&path)?;

    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    let hash = hasher.finalize();
    Ok(format!("{:02x}", hash))
}

pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:02x}", hasher.finalize())
}
