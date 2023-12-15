use std::{collections::HashMap, fs::File, path::Path};

use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Deserialize, Default)]
pub struct InventoryConfig {
    pub hosts: HashMap<String, HostConfig>,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct HostConfig {
    pub host: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub remote_path: Option<String>,
    pub remote_exe: Option<String>,
    pub remote_hpg_exe: Option<String>,
    pub vars_files: Vec<String>,
    pub vars: HashMap<String, String>,
}

impl InventoryConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<InventoryConfig> {
        let p = path.as_ref();
        let config = match p
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .as_deref()
        {
            Some("json") | Some("hjson") => {
                let f = File::open(p)?;
                deser_hjson::from_reader(f)?
            }
            Some("yaml") | Some("yml") => {
                let f = File::open(p)?;
                serde_yaml::from_reader(f)?
            }
            Some(e) => {
                return Err(crate::error::HpgRemoteError::ConfigError(format!(
                    "File extension {}",
                    e
                )));
            }
            None => {
                return Err(crate::error::HpgRemoteError::ConfigError(
                    "No file extension.".into(),
                ));
            }
        };
        Ok(config)
    }

    pub fn config_for_host(&self, host: &str) -> Option<&HostConfig> {
        self.hosts
            .iter()
            .find(|(ref h, _)| h.as_str() == host)
            .map(|(_k, v)| v)
    }
}

impl HostConfig {}
