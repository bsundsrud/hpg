use super::{
    client,
    codec::HpgCodec,
    config::InventoryConfig,
    messages::{FilePatch, PatchType, SyncClientMessage, SyncServerMessage},
};
use crate::{
    error::HpgRemoteError,
    remote::{
        comms::SyncBus,
        messages::{self, FileStatus, HpgMessage},
    },
    HpgOpt,
};
use async_trait::async_trait;
use futures_util::{
    future::join_all, stream::FuturesUnordered, Future, FutureExt, SinkExt, StreamExt,
};
use librsync::whole;
use russh::{
    client::{Handler, Msg},
    Channel, ChannelMsg, Disconnect,
};
use russh_keys::{key, load_secret_key};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    join,
    time::{sleep, timeout},
};
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

pub fn run_hpg_ssh(
    host: HostInfo,
    opt: HpgOpt,
    inventory: InventoryConfig,
) -> Result<(), HpgRemoteError> {
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
    let root_dir = PathBuf::from(opt.config).canonicalize()?;
    let root_dir = root_dir.parent().unwrap();
    let remote_path = host_config
        .and_then(|hc| hc.remote_path.clone())
        .unwrap_or_else(|| {
            format!(
                "/tmp/hpg/{}",
                root_dir
                    .file_name()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            )
        });

    let remote_exe = host_config
        .and_then(|hc| hc.remote_exe.clone())
        .unwrap_or_else(|| "/home/benn/bin/hpg".to_string());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _ = runtime.enter();
    runtime.block_on(async move {
        let ssh_config = load_ssh_config(host, None, None)?;
        let client = Session::connect(ssh_config).await?;
        // client
        //     .sync_files(&root_dir, &remote_path, &remote_exe)
        //     .await?;
        let process = client.start_remote(&remote_path, &remote_exe).await?;
        let socket = client.connect_socket(&root_dir, "/tmp/hpg.socket".to_string());
        let handle = tokio::spawn(async move { process.await });
        //sleep(Duration::from_secs(1)).await;
        socket.await?;
        handle.await.unwrap()?;
        client.close().await?;
        Ok(())
    })
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

    async fn wait_for_socket(&self, socket_path: String) -> Result<Channel<Msg>, HpgRemoteError> {
        loop {
            let res = self
                .session
                .channel_open_direct_streamlocal(&socket_path)
                .await;
            match res {
                Ok(c) => return Ok(c),
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    pub async fn start_remote(
        &self,
        remote_path: &str,
        exe_path: &str,
    ) -> Result<impl Future<Output = Result<(), HpgRemoteError>>, HpgRemoteError> {
        let mut channel = self.session.channel_open_session().await?;
        channel
            .request_pty(false, "xterm", 80, 24, 0, 0, &[])
            .await?;
        let cmdline = format!("{} server {}", exe_path, remote_path);
        eprintln!("Remote cmdline: {}", cmdline);
        channel.exec(true, cmdline).await?;
        eprintln!("started remote");
        let block = async move {
            let mut stdout = tokio::io::stdout();
            let mut stderr = tokio::io::stderr();
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { ref data }) => {
                        stdout.write_all(data).await?;
                        stdout.flush().await?;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        let msg = format!("Exit: {}", exit_status);
                        stdout.write_all(msg.as_bytes()).await?;
                        stdout.flush().await?;
                        channel.eof().await?;
                        break;
                    }
                    Some(ChannelMsg::ExtendedData { ref data, ext }) => {
                        let msg = format!("E{}: ", ext);
                        stderr.write_all(msg.as_bytes()).await?;
                        stderr.write_all(data).await?;
                        stderr.flush().await?;
                    }
                    _ => {}
                }
            }
            Ok(())
        };
        Ok(block)
    }

    pub async fn connect_socket(
        &self,
        root_path: &Path,
        socket_path: String,
    ) -> Result<(), HpgRemoteError> {
        eprintln!("Connecting to remote socket");
        let mut channel =
            match timeout(Duration::from_secs(5), self.wait_for_socket(socket_path)).await {
                Ok(c) => c?,
                Err(_) => {
                    return Err(HpgRemoteError::Unknown(
                        "Timed out waiting for socket".into(),
                    ))
                }
            };
        eprintln!("connected to socket");
        {
            let writer = channel.make_writer();
            let reader = channel.make_reader();
            let bus = SyncBus::new(reader, writer);
            let bus = bus.pin();
            let local_files = client::find_hpg_files(&root_path)?;
            eprintln!("found local files");
            bus.tx(SyncClientMessage::FileList(local_files)).await?;
            eprintln!("Sent message");
            let msg = bus.rx().await?;
            eprintln!("msg back: {:?}", msg);
        }
        channel.eof().await?;
        Ok(())
    }

    pub async fn sync_files(
        &mut self,
        root_path: &Path,
        remote_path: &str,
        exe_path: &str,
    ) -> Result<(), HpgRemoteError> {
        let mut channel = self.session.channel_open_session().await?;
        channel
            .request_pty(false, "xterm", 80, 24, 0, 0, &[])
            .await?;
        let cmdline = format!("{} server {}", exe_path, remote_path);
        eprintln!("Remote cmdline: {}", cmdline);
        channel.exec(true, cmdline).await?;
        let local_files = client::find_hpg_files(&root_path)?;
        let writer = channel.make_writer();
        let reader = channel.make_reader();

        let bus = SyncBus::new(reader, writer);
        let bus = bus.pin();
        bus.tx(HpgMessage::SyncClient(SyncClientMessage::FileList(
            local_files,
        )))
        .await?;
        eprintln!("wrote data");
        let mut patches: HashSet<PathBuf> = HashSet::new();
        let mut started = false;
        loop {
            if patches.is_empty() && started {
                eprintln!("Sending close");
                bus.tx(HpgMessage::SyncClient(SyncClientMessage::Close))
                    .await?;
                break;
            }
            match bus.rx().await? {
                Some(response) => {
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
                                        bus.tx(HpgMessage::SyncClient(patch)).await?;
                                    }
                                    FileStatus::Absent => {
                                        let contents =
                                            read_file_to_bytes(&root_path.join(&file.rel_path))
                                                .await?;
                                        let patch = SyncClientMessage::Patch(FilePatch {
                                            rel_path: file.rel_path,
                                            patch: PatchType::Full { contents },
                                        });
                                        bus.tx(HpgMessage::SyncClient(patch)).await?;
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
                None => {
                    continue;
                }
            }
        }

        Ok(())
    }

    pub async fn close(&self) -> Result<(), HpgRemoteError> {
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
