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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub rel_path: PathBuf,
    pub status: FileStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum FileStatus {
    Present { sig: Vec<u8> },
    Absent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilePatch {
    pub rel_path: PathBuf,
    pub patch: PatchType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PatchType {
    Full { contents: Vec<u8> },
    Partial { delta: Vec<u8> },
}

/*
Message Flow
============

Sync
----

Client              Server
-------------------------------
FileList --->                       Lists local files/dirs waiting to be synced
         <---      FileStatus       Returns file status for to-be-synced files.  Either absent or the file signature
Patch    --->                       Sends deltas for files to server (one message per file)
         <---     PatchApplied      Delta applied successfully (one message per file)

         ***
         <---       Error           If an error happens on the server side, an error with a description will be returned to the client
         <---       Debug           Send message back to client for debugging purposes

         ***
Close    --->                       Sent when sync is done or if an error happens client-side

Exec
----

Client              Server
-------------------------------
Exec     --->                       Run HPG on server side
         <---       Event           Report progress back to client
         <---       Finish          Report done, summary, and success/failure

          **
Cancel   --->                       Cancel current server-side run and exit

*/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SyncClientMessage {
    FileList(Vec<LocalFile>),
    Patch(FilePatch),
    Close,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SyncServerMessage {
    FileStatus(Vec<FileInfo>),
    PatchApplied(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExecClientMessage {
    Exec,
    Cancel,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ExecServerMessage {
    Println(String),
    Event(ServerEvent),
    Finish,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerEvent {
    TaskStart(String),
    BatchStart(u64),
    TaskSuccess,
    TaskSkip,
    TaskFail,
    BatchSuccess,
    BatchFail,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HpgMessage {
    SyncClient(SyncClientMessage),
    SyncServer(SyncServerMessage),
    ExecClient(ExecClientMessage),
    ExecServer(ExecServerMessage),
    Error(String),
    Debug(String),
}

pub fn debug<S: Into<String>>(msg: S) -> HpgMessage {
    HpgMessage::Debug(msg.into())
}
