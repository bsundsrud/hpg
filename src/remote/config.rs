use std::{
    collections::HashMap,
    fs::{self, File},
    path::Path,
};

use serde::Deserialize;

use crate::{error::HpgRemoteError, Result};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct InventoryConfig {
    pub hosts: HashMap<String, HostConfig>,
    pub vars: HashMap<String, toml::Value>,
    pub vars_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct HostConfig {
    pub host: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub sudo: bool,
    pub remote_path: Option<String>,
    pub remote_exe: Option<String>,
    #[serde(default)]
    pub vars_files: Vec<String>,
    #[serde(default)]
    pub vars: HashMap<String, toml::Value>,
}

impl InventoryConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<InventoryConfig, HpgRemoteError> {
        let p = path.as_ref();
        let s = fs::read_to_string(p)?;
        let config = toml::from_str(&s)?;
        Ok(config)
    }

    pub fn config_for_host(&self, host: &str) -> Option<&HostConfig> {
        self.hosts
            .iter()
            .find(|(h, _)| h.as_str() == host)
            .map(|(_k, v)| v)
    }
}

impl HostConfig {}
