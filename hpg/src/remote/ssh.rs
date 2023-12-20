use super::{
    client,
    config::InventoryConfig,
    messages::{FileInfo, FilePatch, PatchType, SyncClientMessage, SyncServerMessage},
};
use crate::{
    debug_output,
    error::HpgRemoteError,
    output,
    remote::{
        comms::SyncBus,
        messages::{FileStatus, HpgMessage},
    },
    task::LuaState,
    tracker::TRACKER,
    HpgOpt,
};
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use futures_util::Future;
use librsync::whole;
use russh::{
    client::{Handler, Msg},
    Channel, ChannelMsg, Disconnect,
};
use russh_keys::{key, load_secret_key};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    time::timeout,
};
use tokio_util::codec::{Decoder, FramedRead, LinesCodec};

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
    let root_dir = PathBuf::from(&opt.config).canonicalize()?;
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
        let process = client.start_remote(&remote_path, &remote_exe).await?;
        let socket = client.connect_socket(&root_dir, "/tmp/hpg.socket".to_string(), opt);
        let handle = tokio::spawn(async move { process.await });
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
                Err(e) => {
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
        let cmdline = format!("{} server {}", exe_path, remote_path);
        debug_output!("Remote cmdline: {}", cmdline);
        channel.exec(true, cmdline).await?;
        let block = async move {
            let mut codec = LinesCodec::new();
            let mut stdout_buf = BytesMut::new();
            let mut stderr_buf = BytesMut::new();
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { ref data }) => {
                        stdout_buf.put(&**data);
                        while let Some(line) = codec.decode(&mut stdout_buf).unwrap() {
                            debug_output!("S: {}", line);
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        debug_output!("Remote process exited: {}", exit_status);
                        break;
                    }
                    Some(ChannelMsg::ExtendedData { ref data, ext }) => {
                        stderr_buf.put(&**data);
                        while let Some(line) = codec.decode(&mut stderr_buf).unwrap() {
                            debug_output!("E: {}", line);
                        }
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
        opts: HpgOpt,
    ) -> Result<(), HpgRemoteError> {
        let mut channel =
            match timeout(Duration::from_secs(5), self.wait_for_socket(socket_path)).await {
                Ok(c) => c?,
                Err(_) => {
                    return Err(HpgRemoteError::Unknown(
                        "Timed out waiting for socket".into(),
                    ))
                }
            };
        sync_files(&mut channel, root_path).await?;
        exec_hpg(&mut channel, root_path, opts).await?;
        channel.eof().await?;
        Ok(())
    }

    pub async fn close(&self) -> Result<(), HpgRemoteError> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await?;
        Ok(())
    }
}

async fn exec_hpg(
    channel: &mut Channel<Msg>,
    root_path: &Path,
    opts: HpgOpt,
) -> Result<(), HpgRemoteError> {
    let writer = channel.make_writer();
    let reader = channel.make_reader();
    let bus = SyncBus::new(reader, writer);
    let bus = bus.pin();

    Ok(())
}

async fn sync_files(channel: &mut Channel<Msg>, root_path: &Path) -> Result<(), HpgRemoteError> {
    let writer = channel.make_writer();
    let reader = channel.make_reader();
    let bus = SyncBus::new(reader, writer);
    let bus = bus.pin();
    let local_files = client::find_hpg_files(&root_path)?;
    bus.tx(SyncClientMessage::FileList(local_files)).await?;
    let msg = bus.rx().await?;
    let mut patches = HashSet::new();
    loop {
        match msg {
            Some(HpgMessage::SyncServer(SyncServerMessage::FileStatus(fi))) => {
                for file in fi {
                    let rel_path = file.rel_path.clone();
                    let f = rel_path.to_string_lossy().to_string();
                    debug_output!("Checking status of {}", f);
                    if let Some(msg) = get_patch_data(root_path, file).await? {
                        patches.insert(rel_path);
                        debug_output!("Pushing patch for {}", f);
                        bus.tx(msg).await?;
                    }
                }
                break;
            }
            Some(HpgMessage::Debug(ref s)) => {
                debug_output!("REMOTE: {}", s);
            }
            Some(HpgMessage::Error(ref e)) => {
                output!("{}", e);
            }
            _ => {
                return Err(HpgRemoteError::Unknown(
                    "out-of-order execution: expected FileStatus".into(),
                ));
            }
        };
    }
    TRACKER.start_progressbar(patches.len() as u64);
    debug_output!("Outstanding patches: {:?}", patches);
    loop {
        if patches.is_empty() {
            bus.tx(HpgMessage::SyncClient(SyncClientMessage::Close))
                .await
                .unwrap();
            break;
        }
        match bus.rx().await? {
            Some(HpgMessage::SyncServer(SyncServerMessage::PatchApplied(p))) => {
                output!("Patched: {}", p.to_string_lossy());
                patches.remove(&p);
                TRACKER.progressbar_progress(format!("Applied: {}", &p.to_string_lossy()));
                debug_output!("Patches left: {:?}", patches);
            }
            Some(HpgMessage::Debug(ref s)) => {
                debug_output!("REMOTE: {}", s);
            }
            Some(HpgMessage::Error(ref e)) => {
                output!("{}", e);
            }
            Some(_) => {
                return Err(HpgRemoteError::Unknown(
                    "out-of-order execution: expected PatchApplied".into(),
                ))
            }
            None => return Err(HpgRemoteError::Unknown("Unexpected end of stream".into())),
        }
    }

    TRACKER.progressbar_finish("Sync Complete".into());
    Ok(())
}

async fn get_patch_data(
    root_path: &Path,
    file: FileInfo,
) -> Result<Option<HpgMessage>, HpgRemoteError> {
    let full_path = root_path.join(&file.rel_path);
    match file.status {
        FileStatus::Present { sig } => {
            let local_sig = super::file_signature(&full_path)?;
            if sig == local_sig {
                debug_output!(
                    "Signatures matched for {}",
                    &file.rel_path.to_string_lossy()
                );
                return Ok(None);
            }
            let delta = generate_delta(&full_path, &sig)?;
            let patch = SyncClientMessage::Patch(FilePatch {
                rel_path: file.rel_path,
                patch: PatchType::Partial { delta },
            });

            Ok(Some(HpgMessage::SyncClient(patch)))
        }
        FileStatus::Absent => {
            let contents = read_file_to_bytes(&root_path.join(&file.rel_path)).await?;
            let patch = SyncClientMessage::Patch(FilePatch {
                rel_path: file.rel_path,
                patch: PatchType::Full { contents },
            });
            Ok(Some(HpgMessage::SyncClient(patch)))
        }
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
