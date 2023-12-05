use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Copy)]
pub enum FileType {
    Dir,
    File,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalFile {
    pub ty: FileType,
    pub rel_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    pub rel_path: PathBuf,
    pub status: FileStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileStatus {
    Present { sig: Vec<u8> },
    Absent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilePatch {
    pub rel_path: PathBuf,
    pub patch: PatchType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PatchType {
    Full { contents: Vec<u8> },
    Partial { delta: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    List(Vec<LocalFile>),
    Info(Vec<FileInfo>),
    Patch(FilePatch),
    PatchApplied(PathBuf),
    Error(String),
    Debug(String),
    Close,
}

pub fn debug<S: Into<String>>(msg: S) -> Message {
    Message::Debug(msg.into())
}
