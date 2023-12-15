use std::{fs::File, path::PathBuf};

use clap::{Parser, Subcommand};
use config::InventoryConfig;
use error::{HpgRemoteError, Result};

mod config;
mod error;
mod local;
mod remote;
mod ssh;
mod transport;
mod types;

#[derive(Debug, Parser)]
#[command(author, version, about = "Run HPG remotely")]
pub(crate) struct Opt {
    #[arg(name = "inventory", short, long, help = "Path to inventory file")]
    inventory: Option<String>,
    #[command(subcommand)]
    commands: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Ssh {
        #[arg(
            name = "[USER@]HOST[:PORT]",
            help = "Remote host address",
            value_parser(try_parse_host)
        )]
        host: HostInfo,
        #[arg(name = "HPG-ARGS", help = "Arguments to hpg")]
        targets: Vec<String>,
    },
    #[command(hide(true))]
    Server {
        #[arg(name = "ROOT-DIR", help = "Base dir for HPG sync")]
        root_dir: String,
    },
}

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub port: Option<u16>,
    pub user: Option<String>,
}

fn try_parse_host(host_str: &str) -> Result<HostInfo> {
    let (user, rest) = if let Some((u, rest)) = host_str.split_once("@") {
        (Some(u.to_string()), rest)
    } else {
        (None, host_str)
    };

    let (hostname, port) = if let Some((h, p)) = rest.split_once(":") {
        let port = Some(p.parse::<u16>().map_err(|_e| HpgRemoteError::ParseHost {
            orig: host_str.to_string(),
            reason: "Could not parse port".into(),
        })?);
        (h.into(), port)
    } else {
        (rest.into(), None)
    };

    Ok(HostInfo {
        hostname,
        port,
        user,
    })
}

fn try_files(paths: &[&str]) -> Result<InventoryConfig> {
    for f in paths {
        let p = PathBuf::from(f);
        if p.exists() {
            return Ok(InventoryConfig::load(p)?);
        }
    }
    Ok(InventoryConfig::default())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opt::parse();
    let inventory = if let Some(p) = opts.inventory {
        try_files(&[&p])?
    } else {
        try_files(&[
            "inventory.yaml",
            "inventory.yml",
            "inventory.hjson",
            "inventory.json",
        ])?
    };
    match opts.commands {
        Commands::Ssh { host, targets: _ } => {
            let host_config = inventory.config_for_host(&host.hostname);
            let host = if let Some(c) = host_config {
                HostInfo {
                    hostname: c.host.clone(),
                    port: c.port.or(host.port),
                    user: c.user.clone().or(host.user),
                }
            } else {
                host
            };
            let ssh_config = ssh::load_ssh_config(host, None, None)?;
            let mut client = ssh::Session::connect(ssh_config).await?;
            let root_path = std::env::current_dir().unwrap().canonicalize()?;

            if let Some(c) = host_config {
                let remote_path: String = if let Some(ref remote_path) = c.remote_path {
                    remote_path.clone()
                } else {
                    format!(
                        "/tmp/hpg/{}",
                        root_path
                            .file_name()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    )
                };

                client
                    .sync_files(&root_path, &remote_path, c.remote_exe.clone())
                    .await?;
            }

            client.close().await?;
        }
        Commands::Server { root_dir } => {
            remote::start_remote(PathBuf::from(root_dir)).await?;
        }
    }

    Ok(())
}
