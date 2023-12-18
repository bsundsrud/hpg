use super::{
    client,
    codec::HpgCodec,
    messages::{FilePatch, PatchType, SyncClientMessage, SyncServerMessage},
};
use crate::{
    error::HpgRemoteError,
    remote::messages::{FileStatus, HpgMessage},
};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use librsync::whole;
use russh::{client::Handler, Disconnect};
use russh_keys::{key, load_secret_key};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{fs::File, io::AsyncReadExt};
use tokio_util::codec::{FramedRead, FramedWrite};

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub port: Option<u16>,
    pub user: Option<String>,
}
pub struct Client {}

#[async_trait]
impl Handler for Client {
    type Error = HpgRemoteError;
    async fn check_server_key(
        self,
        _server_public_key: &key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        Ok((self, true))
    }
}

pub struct Session {
    session: russh::client::Handle<Client>,
}

impl Session {
    pub async fn connect(config: russh_config::Config) -> Result<Self, HpgRemoteError> {
        let key_pair = load_secret_key(config.identity_file.unwrap(), None)?;
        let ssh_config = russh::client::Config {
            inactivity_timeout: Some(Duration::from_secs(5)),
            ..Default::default()
        };
        let ssh_config = Arc::new(ssh_config);
        let sh = Client {};
        let mut session =
            russh::client::connect(ssh_config, (config.host_name, config.port), sh).await?;
        let auth_res = session
            .authenticate_publickey(&config.user, Arc::new(key_pair))
            .await?;

        if !auth_res {
            return Err(HpgRemoteError::AuthFailed(config.user));
        }
        Ok(Self { session })
    }

    pub async fn sync_files(
        &mut self,
        root_path: &Path,
        remote_path: &str,
        exe_path: Option<String>,
    ) -> Result<(), HpgRemoteError> {
        let mut channel = self.session.channel_open_session().await?;
        let exe_path = if let Some(ref s) = exe_path {
            &s
        } else {
            "hpg"
        };
        let cmdline = format!("{} server {}", exe_path, remote_path);
        eprintln!("Remote cmdline: {}", cmdline);
        channel.exec(true, cmdline).await?;
        let local_files = client::find_hpg_files(&root_path)?;
        let encoder: HpgCodec<HpgMessage> = HpgCodec::new();
        let decoder: HpgCodec<HpgMessage> = HpgCodec::new();
        let mut hpg_writer = FramedWrite::new(channel.make_writer(), encoder);
        let mut hpg_reader = FramedRead::new(channel.make_reader(), decoder);

        hpg_writer
            .send(HpgMessage::SyncClient(SyncClientMessage::FileList(
                local_files,
            )))
            .await?;
        eprintln!("wrote data");
        let mut patches: HashSet<PathBuf> = HashSet::new();
        let mut started = false;
        loop {
            if patches.is_empty() && started {
                eprintln!("Sending close");
                hpg_writer
                    .send(HpgMessage::SyncClient(SyncClientMessage::Close))
                    .await?;
                break;
            }
            match hpg_reader.next().await {
                Some(Ok(response)) => {
                    eprintln!("got response {:?}", response);
                    match response {
                        HpgMessage::SyncServer(SyncServerMessage::FileStatus(i)) => {
                            for file in i {
                                patches.insert(file.rel_path.clone());
                                started = true;
                                let full_path = root_path.join(&file.rel_path);
                                match file.status {
                                    FileStatus::Present { sig } => {
                                        let delta = generate_delta(&full_path, &sig)?;
                                        eprintln!("Calculated delta, {} bytes", delta.len());
                                        let patch = SyncClientMessage::Patch(FilePatch {
                                            rel_path: file.rel_path,
                                            patch: PatchType::Partial { delta },
                                        });
                                        hpg_writer.send(HpgMessage::SyncClient(patch)).await?;
                                    }
                                    FileStatus::Absent => {
                                        let contents =
                                            read_file_to_bytes(&root_path.join(&file.rel_path))
                                                .await?;
                                        let patch = SyncClientMessage::Patch(FilePatch {
                                            rel_path: file.rel_path,
                                            patch: PatchType::Full { contents },
                                        });
                                        hpg_writer.send(HpgMessage::SyncClient(patch)).await?;
                                    }
                                }
                            }
                        }
                        HpgMessage::Error(e) => {
                            return Err(HpgRemoteError::Unknown(format!(
                                "Error during sync: {}",
                                e
                            )));
                        }
                        HpgMessage::Debug(s) => {
                            eprintln!("REMOTE: {}", s);
                        }
                        HpgMessage::SyncServer(SyncServerMessage::PatchApplied(p)) => {
                            eprintln!("Applied patch to {:?}", p);
                            patches.remove(&p);
                        }
                        _ => {
                            // We don't handle other message types
                            continue;
                        }
                    }
                }
                Some(Err(e)) => {
                    eprintln!("Got Error: {}", e);
                    return Err(e);
                }
                None => continue,
            }
        }

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), HpgRemoteError> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}

fn generate_delta(local_path: &Path, remote_sig: &[u8]) -> Result<Vec<u8>, HpgRemoteError> {
    let f = std::fs::File::open(local_path)?;
    let mut reader = std::io::BufReader::new(f);
    let mut sig_reader = std::io::BufReader::new(remote_sig);
    let mut delta = Vec::new();
    whole::delta(&mut reader, &mut sig_reader, &mut delta)?;
    Ok(delta)
}

async fn read_file_to_bytes(path: &Path) -> Result<Vec<u8>, HpgRemoteError> {
    let mut file = File::open(path).await?;

    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;
    Ok(contents)
}

fn default_ssh_dir() -> Result<PathBuf, HpgRemoteError> {
    let mut home = if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err(HpgRemoteError::MissingKeyError(
            "Could not load default ssh identity".into(),
        ));
    };
    home.push(".ssh");
    Ok(home)
}

fn default_ssh_config(host: &str) -> Option<russh_config::Config> {
    default_ssh_dir()
        .ok()
        .map(|dir| dir.join("config"))
        .filter(|file| file.exists() && file.is_file())
        .map(|file| russh_config::parse_path(&file, host).ok())
        .flatten()
}

fn default_ssh_identity() -> Option<String> {
    default_ssh_dir()
        .ok()
        .map(|dir| dir.join("id_rsa"))
        .filter(|id| id.exists() && id.is_file())
        .map(|identity| identity.to_string_lossy().to_string())
}

pub fn load_ssh_config(
    hostinfo: HostInfo,
    config_path: Option<&Path>,
    identity_file: Option<String>,
) -> Result<russh_config::Config, HpgRemoteError> {
    let mut config = if let Some(config_path) = config_path {
        russh_config::parse_path(config_path, &hostinfo.hostname)?
    } else if let Some(config) = default_ssh_config(&hostinfo.hostname) {
        config
    } else {
        russh_config::Config::default(&hostinfo.hostname)
    };

    if let Some(p) = hostinfo.port {
        config.port = p;
    }
    if let Some(u) = hostinfo.user {
        config.user = u;
    }
    if let Some(p) = identity_file {
        config.identity_file = Some(p);
    }
    if config.identity_file.is_none() {
        if let Some(identity) = default_ssh_identity() {
            config.identity_file = Some(identity);
        } else {
            return Err(HpgRemoteError::MissingKeyError(
                "No identity file provided or found.".into(),
            ));
        }
    }
    Ok(config)
}
