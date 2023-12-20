use std::{fs::File, io::BufReader, path::Path};

use librsync::whole;

use crate::error::HpgRemoteError;

pub mod client;
pub mod codec;
pub mod comms;
pub mod config;
pub mod messages;
pub mod server;
pub mod ssh;

pub fn file_signature(path: &Path) -> Result<Vec<u8>, HpgRemoteError> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut sig = Vec::new();
    {
        whole::signature(&mut reader, &mut sig)?;
    }
    Ok(sig)
}
