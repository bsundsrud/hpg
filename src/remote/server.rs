use std::{
    io::{BufReader, BufWriter},
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
use crate::{
    error::HpgRemoteError,
    load_file, output,
    remote::messages::ExecServerMessage,
    task::{LuaState, Variables},
    tracker::{self, Tracker},
};
use futures_util::{SinkExt, StreamExt};
use librsync::whole;
use nix::unistd::{Gid, Uid};
use tokio::{
    fs::OpenOptions,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    time,
};
use tokio_util::codec::Framed;

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

pub fn run_socket_server(
    root_dir: String,
    lua: LuaState,
    socket_path: &Path,
) -> Result<(), HpgRemoteError> {
    let root_dir = PathBuf::from(root_dir);
    if !root_dir.exists() {
        std::fs::create_dir_all(&root_dir)?;
    }
    std::env::set_current_dir(&root_dir)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _ = rt.enter();
    rt.block_on(async move {
        listen_socket(socket_path, &root_dir, lua).await?;
        if socket_path.exists() {
            tokio::fs::remove_file(&socket_path).await?;
        }
        Ok(())
    })
}

async fn listen_socket(
    socket_path: &Path,
    root_dir: &Path,
    lua: LuaState,
) -> Result<(), HpgRemoteError> {
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
            eprintln!("SERVER: Timed out waiting for connection");
            return Ok(());
        }
    };
    let (stream, _addr) = res?;
    let mut rw = Framed::new(stream, HpgCodec::<HpgMessage>::new());
    server_sync(root_dir, &mut rw).await?;

    eprintln!("SERVER: sync done");

    server_exec_hpg(root_dir, lua, rw).await?;
    // moving to execution mode now
    Ok(())
}

async fn server_sync<R: AsyncRead + AsyncWrite + Unpin>(
    root_dir: &Path,
    rw: &mut Framed<R, HpgCodec<HpgMessage>>,
) -> Result<(), HpgRemoteError> {
    loop {
        let msg = match time::timeout(Duration::from_secs(50), rw.next()).await {
            Ok(m) => m,
            Err(_e) => {
                eprintln!("SERVER: Timed out");
                break;
            }
        };
        let msg = if let Some(m) = msg {
            m
        } else {
            eprintln!("SERVER: Stream closed");
            break;
        };
        let msg = msg?;
        match msg {
            HpgMessage::SyncClient(SyncClientMessage::FileList(list)) => {
                let info = check_dir(&root_dir, &list)?;
                rw.send(HpgMessage::SyncServer(SyncServerMessage::FileStatus(info)))
                    .await?;
                rw.send(debug("sent file status")).await?;
            }
            HpgMessage::SyncClient(SyncClientMessage::Patch(p)) => {
                let path = root_dir.join(&p.rel_path);
                rw.send(debug(format!("applying patch for {:?}", &path)))
                    .await?;
                match p.patch {
                    PatchType::Full { contents } => {
                        let mut f = OpenOptions::new()
                            .truncate(true)
                            .create(true)
                            .write(true)
                            .open(&path)
                            .await?;
                        f.write_all(&contents).await?;
                    }
                    PatchType::Partial { delta } => apply_patch(&path, &delta)?,
                }
                rw.send(HpgMessage::SyncServer(SyncServerMessage::PatchApplied(
                    p.rel_path.clone(),
                )))
                .await?;
            }
            HpgMessage::SyncClient(SyncClientMessage::Close) => {
                break;
            }
            _ => continue,
        }
    }
    Ok(())
}

async fn server_exec_hpg(
    root_dir: &Path,
    lua: LuaState,
    mut rw: Framed<UnixStream, HpgCodec<HpgMessage>>,
) -> Result<(), HpgRemoteError> {
    match wait_for_exec_msg(&mut rw).await? {
        HpgMessage::ExecClient {
            vars,
            config,
            run_defaults,
            show_plan,
            targets,
        } => {
            tracker::sink().into_remote(rw);
            if let Err(e) = execute_hpg(lua, config, vars, run_defaults, show_plan, targets).await {
                output!("Remote error: {}", e);
            }
            rw = tracker::sink().into_local().unwrap();
        }
        _ => unreachable!(),
    }

    rw.send(HpgMessage::ExecServer(ExecServerMessage::Finish))
        .await?;
    Ok(())
}

async fn wait_for_exec_msg(
    rw: &mut Framed<UnixStream, HpgCodec<HpgMessage>>,
) -> Result<HpgMessage, HpgRemoteError> {
    loop {
        let msg = match time::timeout(Duration::from_secs(50), rw.next()).await {
            Ok(m) => m,
            Err(_e) => {
                return Err(HpgRemoteError::Unknown("SERVER: Timed out".into()));
            }
        };

        let msg = if let Some(m) = msg {
            m
        } else {
            return Err(HpgRemoteError::Unknown("SERVER: Stream closed".into()));
        };
        let msg = msg?;
        match msg {
            HpgMessage::ExecClient { .. } => return Ok(msg),
            _ => continue,
        }
    }
}

async fn execute_hpg(
    lua: LuaState,
    config: String,
    vars: Variables,
    run_defaults: bool,
    show_plan: bool,
    targets: Vec<String>,
) -> Result<(), HpgRemoteError> {
    tracker::tracker().run(5);
    tokio::time::sleep(Duration::from_secs(1)).await;
    output!("Config: {}, Targets: {:?}", config, targets);
    tokio::time::sleep(Duration::from_secs(1)).await;
    // let code = load_file(&config)?;

    // let lua = lua.eval(&code, vars)?;
    // if opt.list {
    //     output!("{}", style("Available Tasks").cyan());
    //     for (name, task) in lua.available_targets() {
    //         indent_output!(1, "{}: {}", style(name).green(), task.description());
    //     }
    //     return Ok(());
    // }
    // let requested_tasks: Vec<&str> = opt.targets.iter().map(|t| t.as_str()).collect();
    // lua.execute(&requested_tasks, opt.run_defaults, opt.show)?;
    Ok(())
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
                    let fi = FileInfo {
                        rel_path: f.rel_path.clone(),
                        status: FileStatus::Present {
                            sig: super::file_signature(&full_path)?,
                        },
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
