use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use fast_rsync::{Signature, SignatureOptions};

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
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    let sig = Signature::calculate(
        &buf,
        SignatureOptions {
            block_size: 4096,
            crypto_hash_size: 8,
        },
    );
    let s = sig.into_serialized();
    Ok(s)
}
