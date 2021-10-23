use thiserror::Error;

use crate::tasks::TaskRef;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    CycleError(TaskRef),
    #[error("Unknown task {0}")]
    UnknownTask(TaskRef),
    #[error("Lua Error: {0}")]
    LuaError(#[from] rlua::Error),
}

#[derive(Debug, Error)]
pub enum HpgError {
    #[error("Task Error: {0}")]
    TaskError(#[from] TaskError),
    #[error("File Error: {0}")]
    FileError(#[from] std::io::Error),
}
