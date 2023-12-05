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
    #[error("File Error: {0}")]
    File(#[from] std::io::Error),
    #[error("Parse Error: {0}")]
    Parse(String),
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
