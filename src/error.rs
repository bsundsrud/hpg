use std::sync::Arc;

use thiserror::Error;

use crate::task::TaskHandle;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    CycleError(TaskHandle),
    #[error("Unknown task {0}")]
    UnknownTask(TaskHandle),
    #[error("Lua Error: {0}")]
    LuaError(#[from] mlua::Error),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Action Failed: {0}")]
    ActionError(String),
    #[error("A task was skipped")]
    SkippedTask,
    #[error("Templating error: {0}")]
    TemplateError(#[from] tera::Error),
    #[error("Dbus error: {0}")]
    DbusError(#[from] zbus::Error),
}

#[derive(Debug, Error)]
pub enum HpgError {
    #[error("Task Error: {0}")]
    TaskError(#[from] TaskError),
    #[error("File Error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Parse Error: {0}")]
    ParseError(String),
}

pub(crate) fn action_error<S: Into<String>>(msg: S) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(TaskError::ActionError(msg.into())))
}

pub(crate) fn task_error(err: TaskError) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(err))
}

pub(crate) fn io_error(e: std::io::Error) -> mlua::Error {
    mlua::Error::ExternalError(Arc::new(TaskError::IoError(e)))
}
