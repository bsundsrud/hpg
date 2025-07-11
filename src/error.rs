use std::sync::Arc;

use thiserror::Error;

use crate::task::TaskHandle;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    Cycle(TaskHandle),
    #[error("Unknown task {0}")]
    UnknownTask(TaskHandle),
    #[error("Lua Error: {0}")]
    Lua(#[from] mlua::Error),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Action Failed: {0}")]
    Action(String),
    #[error("A task was skipped")]
    SkippedTask,
    #[error("Templating error: {0}")]
    Template(#[from] tera::Error),
    #[error("Dbus error: {0}")]
    Dbus(#[from] zbus::Error),
}

#[derive(Debug, Error)]
pub enum HpgError {
    #[error("Task Error: {0}")]
    Task(#[from] TaskError),
    #[error("Remote Error: {0}")]
    Remote(#[from] HpgRemoteError),
    #[error("File Error: {0}")]
    File(#[from] std::io::Error),
    #[error("Parse Error: {0}")]
    Parse(String),
    #[error("Serialization Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub(crate) fn action_error<S: Into<String>>(msg: S) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(TaskError::Action(msg.into())))
}

pub(crate) fn task_error(err: TaskError) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(err))
}

pub(crate) fn io_error(e: std::io::Error) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(TaskError::Io(e)))
}

#[derive(Debug, Error)]
pub enum HpgRemoteError {
    #[error("Authentication failed for user {0}")]
    AuthFailed(String),
    #[error("SSH Error: {0}")]
    SshError(#[from] russh::Error),
    #[error("Missing Identity: {0}")]
    MissingKeyError(String),
    #[error("Key Error: {0}")]
    KeyError(#[from] russh::keys::Error),
    #[error("Could not parse SSH config: {0}")]
    ParseConfig(#[from] russh_config::Error),
    #[error("Could not parse SSH host address '{orig}': {reason}")]
    ParseHost { orig: String, reason: String },
    #[error("I/O Error: {error}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
    #[error("Ignore file error: {0}")]
    IgnoreError(#[from] ignore::Error),
    #[error("Error serializing client/server communications: {0}")]
    SerilizationError(#[from] ciborium::ser::Error<std::io::Error>),
    #[error("Error deserializing client/server communications: {0}")]
    DeserilizationError(#[from] ciborium::de::Error<std::io::Error>),
    #[error("Error deserializing inventory config: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Error deserializing inventory config: {0}")]
    InventoryError(#[from] toml::de::Error),
    #[error("Unknown inventory format: {0}")]
    ConfigError(String),
    #[error("Rsync apply error: {0}")]
    RsyncApplyError(#[from] fast_rsync::ApplyError),
    #[error("Rsync sig error: {0}")]
    RsyncSigError(#[from] fast_rsync::SignatureParseError),
    #[error("Rsync diff error: {0}")]
    RsyncDiffError(#[from] fast_rsync::DiffError),
    #[error("Unknown Error: {0}")]
    Unknown(String),
    #[error("Exec Error: {0}")]
    ExecError(#[from] Box<HpgError>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
