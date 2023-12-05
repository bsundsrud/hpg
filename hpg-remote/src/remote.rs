use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use librsync::whole;
use tokio::{
    fs::OpenOptions,
    io::{AsyncWriteExt, Stdout},
    time,
};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{
    error::{HpgRemoteError, Result},
    transport::HpgCodec,
    types::{debug, FileInfo, FileStatus, FileType, LocalFile, Message, PatchType},
};

pub async fn start_remote(root_dir: PathBuf) -> Result<()> {
    if !root_dir.exists() {
        std::fs::create_dir_all(&root_dir)?;
    }
    let input = tokio::io::stdin();
    let output = tokio::io::stdout();

    let mut hpg_reader = FramedRead::new(input, HpgCodec {});
    let mut hpg_writer = FramedWrite::new(output, HpgCodec {});
    hpg_writer.send(debug("Started")).await?;
    loop {
        match time::timeout(Duration::from_secs(5), hpg_reader.next()).await {
            Ok(Some(Ok(msg))) => match handle_message(msg, &root_dir, &mut hpg_writer).await {
                Ok(None) => break,
                Ok(Some(_)) => continue,
                Err(e) => {
                    hpg_writer.send(Message::Error(e.to_string())).await?;
                }
            },
            Ok(Some(Err(e))) => {
                hpg_writer.send(Message::Error(e.to_string())).await?;
                return Err(e);
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    hpg_writer.send(debug("Shutdown")).await?;
    Ok(())
}

async fn handle_message(
    msg: Message,
    root_dir: &Path,
    writer: &mut FramedWrite<Stdout, HpgCodec>,
) -> Result<Option<()>> {
    writer
        .send(debug(format!("Received message: {:?}", msg)))
        .await?;
    match msg {
        Message::List(l) => {
            let info = check_dir(&root_dir, &l);
            writer
                .send(debug(format!("Calculated: {:?}", info)))
                .await?;
            let info = info?;
            writer.send(Message::Info(info)).await?;
            writer.send(debug("sent fileinfo")).await?;
        }
        Message::Info(_) => {
            // Server should not receive Info messages
            return Err(HpgRemoteError::Unknown("Invalid message type Info".into()));
        }
        Message::Patch(p) => {
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
            writer.send(Message::PatchApplied(p.rel_path)).await?;
        }
        Message::Error(e) => {
            // On the server side, there's no error recovery we can do. Just exit
            return Err(HpgRemoteError::Unknown(e));
        }
        Message::Debug(_) => {
            unreachable!("Remote does not handle Debug");
        }
        Message::PatchApplied(_) => {
            unreachable!("Remote does not handle PatchApplied");
        }
        Message::Close => return Ok(None),
    }
    Ok(Some(()))
}

fn apply_patch(path: &Path, patch: &[u8]) -> Result<()> {
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

fn check_dir(root_path: &Path, files: &[LocalFile]) -> Result<Vec<FileInfo>> {
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
