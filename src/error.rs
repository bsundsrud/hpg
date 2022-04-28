use crate::tasks::TaskRef;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    CycleError(TaskRef),
    #[error("Unknown task {0}")]
    UnknownTask(TaskRef),
    #[error("Lua Error: {0}")]
    LuaError(#[from] rlua::Error),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Action Failed: {0}")]
    ActionError(String),
    #[error("A task was skipped")]
    SkippedTask,
    #[error("Templating error: {0}")]
    TemplateError(#[from] tera::Error),
}

#[derive(Debug, Error)]
pub enum HpgError {
    #[error("Task Error: {0}")]
    TaskError(#[from] TaskError),
    #[error("File Error: {0}")]
    FileError(#[from] std::io::Error),
}
