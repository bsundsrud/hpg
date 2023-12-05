use std::path::PathBuf;

use error::{HpgRemoteError, Result};
use structopt::StructOpt;

mod error;
mod local;
mod remote;
mod ssh;
mod transport;
mod types;

#[derive(Debug, StructOpt)]
#[structopt(name = "hpg-remote", about = "Run HPG remotely")]
pub(crate) enum Opt {
    Server(ServerOpts),
    Push(LocalOpts),
}

#[derive(Debug, StructOpt)]
pub struct LocalOpts {
    #[structopt(name = "[USER@]HOST[:PORT]", help = "Remote host address", parse(try_from_str = try_parse_host))]
    host: HostInfo,
    #[structopt(name = "TARGETS", help = "Tasks to run on remote host")]
    targets: Vec<String>,
}

#[derive(Debug, StructOpt)]
pub struct ServerOpts {
    #[structopt(name = "ROOT-DIR", help = "Base dir for HPG sync")]
    root_dir: String,
}

#[derive(Debug)]
pub struct HostInfo {
    pub hostname: String,
    pub port: Option<u16>,
    pub user: Option<String>,
}

fn try_parse_host(host_str: &str) -> Result<HostInfo> {
    let mut user = None;
    let mut hostname = String::new();
    let mut port = None;
    let rest = if let Some((u, rest)) = host_str.split_once("@") {
        user = Some(u.to_string());
        rest
    } else {
        host_str
    };

    if let Some((h, p)) = rest.split_once(":") {
        hostname = h.into();
        port = Some(p.parse::<u16>().map_err(|_e| HpgRemoteError::ParseHost {
            orig: host_str.to_string(),
            reason: "Could not parse port".into(),
        })?);
    } else {
        hostname = rest.into();
    }

    //TODO: Actually parse
    Ok(HostInfo {
        hostname,
        port,
        user,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    match Opt::from_args() {
        Opt::Push(opts) => {
            let ssh_config = dbg!(ssh::load_ssh_config(opts.host, None, None)?);
            let mut client = ssh::Session::connect(ssh_config).await?;

            client
                .open_remote(&std::env::current_dir().unwrap(), None)
                .await?;
            client.close().await?;
        }
        Opt::Server(opts) => {
            remote::start_remote(PathBuf::from(opts.root_dir)).await?;
        }
    }

    Ok(())
}
