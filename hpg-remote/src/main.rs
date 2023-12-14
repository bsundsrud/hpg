use std::path::PathBuf;

use clap::Parser;
use error::{HpgRemoteError, Result};

mod error;
mod local;
mod remote;
mod ssh;
mod transport;
mod types;

#[derive(Debug, Parser)]
#[command(author, version, about = "Run HPG remotely")]
pub(crate) enum Opt {
    #[command(hide(true))]
    Server(ServerOpts),
    Ssh(LocalOpts),
}

#[derive(Debug, Parser)]
#[command(trailing_var_arg(true))]
pub struct LocalOpts {
    #[arg(
        name = "[USER@]HOST[:PORT]",
        help = "Remote host address",
        value_parser(try_parse_host)
    )]
    host: HostInfo,
    #[arg(name = "Remote path to config dir", short = 'p', long)]
    remote_path: Option<String>,
    #[arg(name = "HPG-ARGS", help = "Arguments to hpg")]
    targets: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct ServerOpts {
    #[arg(name = "ROOT-DIR", help = "Base dir for HPG sync")]
    root_dir: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    match Opt::parse() {
        Opt::Ssh(opts) => {
            let ssh_config = dbg!(ssh::load_ssh_config(opts.host, None, None)?);
            let mut client = ssh::Session::connect(ssh_config).await?;
            let root_path = std::env::current_dir().unwrap().canonicalize()?;
            let remote_path: String = if let Some(remote_path) = opts.remote_path {
                remote_path
            } else {
                format!(
                    "/tmp/hpg/{}",
                    root_path
                        .file_name()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                )
            };
            client.sync_files(&root_path, &remote_path, None).await?;
            client.close().await?;
        }
        Opt::Server(opts) => {
            remote::start_remote(PathBuf::from(opts.root_dir)).await?;
        }
    }

    Ok(())
}
