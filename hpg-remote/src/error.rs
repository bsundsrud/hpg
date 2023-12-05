use thiserror::Error;
pub type Result<T, E = HpgRemoteError> = core::result::Result<T, E>;
#[derive(Debug, Error)]
pub enum HpgRemoteError {
    #[error("Authentication failed for user {0}")]
    AuthFailed(String),
    #[error("SSH Error: {0}")]
    SshError(#[from] russh::Error),
    #[error("Missing Identity: {0}")]
    MissingKeyError(String),
    #[error("Key Error: {0}")]
    KeyError(#[from] russh_keys::Error),
    #[error("Could not parse SSH config: {0}")]
    ParseConfig(#[from] russh_config::Error),
    #[error("Could not parse SSH host address '{orig}': {reason}")]
    ParseHost { orig: String, reason: String },
    #[error("I/O Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Ignore file error: {0}")]
    IgnoreError(#[from] ignore::Error),
    #[error("Error serializing client/server communications: {0}")]
    SerilizationError(#[from] ciborium::ser::Error<std::io::Error>),
    #[error("Error deserializing client/server communications: {0}")]
    DeserilizationError(#[from] ciborium::de::Error<std::io::Error>),
    #[error("Rsync calculation error: {0}")]
    RsyncError(#[from] librsync::Error),
    #[error("Unknown Error: {0}")]
    Unknown(String),
}
