use std::{
    fs::{File, Permissions},
    io::{BufReader, BufWriter, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::fs::PermissionsExt,
    },
    path::{Path, PathBuf},
    time::Duration,
};

use super::{
    codec::HpgCodec,
    messages::{
        debug, FileInfo, FileStatus, FileType, HpgMessage, LocalFile, PatchType, SyncClientMessage,
        SyncServerMessage,
    },
};
use crate::{error::HpgRemoteError, remote::comms::SyncBus, task::LuaState};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use librsync::whole;
use nix::{
    sys::stat::{fchmod, Mode},
    unistd::{fchown, Gid, Uid},
};
use tokio::{
    fs::OpenOptions,
    io::{AsyncWriteExt, Stdin, Stdout},
    net::UnixListener,
    time,
};
use tokio_util::{
    bytes::BytesMut,
    codec::{Encoder, Framed, FramedRead, FramedWrite},
};

fn running_as_sudo() -> (bool, Uid, Gid) {
    let current_uid = nix::unistd::getuid();
    let current_gid = nix::unistd::getgid();
    let is_root = current_uid.is_root();
    if let Some(v) = std::env::var_os("SUDO_UID") {
        let uid: u32 = v.to_string_lossy().parse().unwrap();
        let sudo_uid = Uid::from(uid);
        if current_uid != sudo_uid {
            let gid: u32 = std::env::var("SUDO_GID").unwrap().parse().unwrap();
            return (true, sudo_uid, Gid::from(gid));
        } else {
            return (false, current_uid, current_gid);
        }
    } else {
        (is_root, current_uid, current_gid)
    }
}

async fn listen_socket(socket_path: &Path, root_dir: &Path) -> Result<(), HpgRemoteError> {
    if socket_path.exists() {
        tokio::fs::remove_file(&socket_path).await?;
    }
    let listener = UnixListener::bind(socket_path)?;
    if let (true, uid, gid) = running_as_sudo() {
        std::os::unix::fs::chown(&socket_path, Some(uid.as_raw()), Some(gid.as_raw())).unwrap();
    }

    // should wait for client to connect
    let res = match time::timeout(Duration::from_secs(5), listener.accept()).await {
        Ok(r) => r,
        Err(_e) => {
            eprintln!("Timed out waiting for connection");
            return Ok(());
        }
    };
    let (stream, _addr) = res?;
    let mut rw = Framed::new(stream, HpgCodec::<HpgMessage>::new());
    match time::timeout(Duration::from_secs(5), rw.next()).await {
        Ok(Some(Ok(HpgMessage::SyncClient(SyncClientMessage::FileList(list))))) => {
            let info = check_dir(&root_dir, &list)?;
            rw.send(HpgMessage::SyncServer(SyncServerMessage::FileStatus(info)))
                .await?;
        }
        Ok(Some(Ok(m))) => {
            //out of sequence message
            println!("unknown message: {:?}", m);
            return Ok(());
        }
        Ok(Some(Err(e))) => {
            println!("error: {}", e);
            return Err(e);
        }
        Ok(None) => {
            // closed stream
            println!("Stream closed");
            return Ok(());
        }
        Err(_) => {
            //timeout
            println!("Timed out");
            return Ok(());
        }
    }
    println!("Exiting");
    Ok(())
}

pub fn run_socket_server(
    root_dir: String,
    lua: LuaState,
    socket_path: &Path,
) -> Result<(), HpgRemoteError> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        listen_socket(socket_path, &PathBuf::from(root_dir)).await?;
        if socket_path.exists() {
            tokio::fs::remove_file(&socket_path).await?;
        }
        Ok(())
    })
}

pub fn run_hpg_server(root_dir: String, lua: LuaState) {
    let root_path = PathBuf::from(root_dir);
    let mut encoder: HpgCodec<HpgMessage> = HpgCodec::new();
    if !root_path.exists() {
        std::fs::create_dir_all(&root_path).unwrap();
    }
    if let Err(e) = std::env::set_current_dir(&root_path) {
        let mut bytes = BytesMut::new();
        encoder
            .encode(HpgMessage::Error(e.to_string()), &mut bytes)
            .unwrap();
        std::io::stdout().write_all(&bytes).unwrap();
        return;
    }
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let mut bytes = BytesMut::new();
            encoder
                .encode(HpgMessage::Error(e.to_string()), &mut bytes)
                .unwrap();
            std::io::stdout().write_all(&bytes).unwrap();
            return;
        }
    };
    let res = rt.block_on(async move {
        let input = tokio::io::stdin();
        let output = tokio::io::stdout();

        let decoder: HpgCodec<HpgMessage> = HpgCodec::new();
        let encoder: HpgCodec<HpgMessage> = HpgCodec::new();

        let hpg_reader = FramedRead::new(input, decoder);
        let hpg_writer = FramedWrite::new(output, encoder);
        start_remote_sync(root_path.clone(), hpg_reader, hpg_writer).await
    });

    if let Err(e) = res {
        let mut bytes = BytesMut::new();
        encoder
            .encode(HpgMessage::Error(e.to_string()), &mut bytes)
            .unwrap();
        std::io::stdout().write_all(&bytes).unwrap();
        return;
    }
}

pub async fn start_remote_sync(
    root_dir: PathBuf,
    mut reader: FramedRead<Stdin, HpgCodec<HpgMessage>>,
    mut writer: FramedWrite<Stdout, HpgCodec<HpgMessage>>,
) -> Result<(), HpgRemoteError> {
    if !root_dir.exists() {
        std::fs::create_dir_all(&root_dir)?;
    }
    writer.send(debug("Started")).await?;
    loop {
        match time::timeout(Duration::from_secs(500), reader.next()).await {
            Ok(Some(Ok(HpgMessage::SyncClient(msg)))) => {
                match handle_message(msg, &root_dir, &mut writer).await {
                    Ok(None) => break,
                    Ok(Some(_)) => continue,
                    Err(e) => {
                        writer.send(HpgMessage::Error(e.to_string())).await?;
                    }
                }
            }
            Ok(Some(Ok(_))) => {
                // we don't handle any other message types here
                continue;
            }
            Ok(Some(Err(e))) => {
                writer.send(HpgMessage::Error(e.to_string())).await?;
                return Err(e);
            }
            Ok(None) => {
                continue;
            }
            Err(_) => {
                break;
            }
        }
    }
    writer.send(debug("Shutdown")).await?;
    Ok(())
}

async fn handle_message(
    msg: SyncClientMessage,
    root_dir: &Path,
    writer: &mut FramedWrite<Stdout, HpgCodec<HpgMessage>>,
) -> Result<Option<()>, HpgRemoteError> {
    writer
        .send(debug(format!("Received message: {:?}", msg)))
        .await?;
    match msg {
        SyncClientMessage::FileList(l) => {
            let info = check_dir(&root_dir, &l);
            writer
                .send(debug(format!("Calculated: {:?}", info)))
                .await?;
            let info = info?;
            writer
                .send(HpgMessage::SyncServer(SyncServerMessage::FileStatus(info)))
                .await?;
            writer.send(debug("sent fileinfo")).await?;
        }
        SyncClientMessage::Patch(p) => {
            let full_path = root_dir.join(&p.rel_path);
            match p.patch {
                PatchType::Full { contents } => {
                    let mut f = OpenOptions::new()
                        .truncate(true)
                        .create(true)
                        .write(true)
                        .open(&full_path)
                        .await?;
                    f.write_all(&contents).await?;
                }
                PatchType::Partial { delta } => {
                    apply_patch(&full_path, &delta)?;
                }
            }
            writer
                .send(HpgMessage::SyncServer(SyncServerMessage::PatchApplied(
                    p.rel_path,
                )))
                .await?;
        }
        SyncClientMessage::Close => return Ok(None),
    }
    Ok(Some(()))
}

fn apply_patch(path: &Path, patch: &[u8]) -> Result<(), HpgRemoteError> {
    let temp_path = PathBuf::from(format!("{}.hpg-sync", path.to_string_lossy()));
    {
        let temp_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        let orig_file = std::fs::File::open(&path)?;
        let mut reader = BufReader::new(orig_file);
        let mut patch_reader = BufReader::new(patch);
        let mut writer = BufWriter::new(temp_file);

        whole::patch(&mut reader, &mut patch_reader, &mut writer)?;
    }
    std::fs::remove_file(path)?;
    std::fs::rename(&temp_path, &path)?;
    Ok(())
}

fn check_dir(root_path: &Path, files: &[LocalFile]) -> Result<Vec<FileInfo>, HpgRemoteError> {
    let mut results = Vec::new();
    for f in files {
        let full_path = root_path.join(&f.rel_path);
        match f.ty {
            FileType::Dir => {
                if !full_path.exists() {
                    std::fs::create_dir_all(&full_path)?;
                }
            }
            FileType::File => {
                if full_path.exists() {
                    let file = File::open(full_path)?;
                    let mut reader = BufReader::new(file);
                    let mut sig = Vec::new();
                    {
                        whole::signature(&mut reader, &mut sig)?;
                    }
                    let fi = FileInfo {
                        rel_path: f.rel_path.clone(),
                        status: FileStatus::Present { sig },
                    };
                    results.push(fi);
                } else {
                    results.push(FileInfo {
                        rel_path: f.rel_path.clone(),
                        status: FileStatus::Absent,
                    });
                }
            }
        }
    }
    Ok(results)
}
