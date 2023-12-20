use std::{
    pin::Pin,
    sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}},
    time::Duration,
};

use crate::error::HpgRemoteError;

use super::{codec::HpgCodec, messages::HpgMessage};
use futures_util::{SinkExt, StreamExt};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    time, fs::File,
};
use tokio_util::{codec::{FramedRead, FramedWrite, Encoder}, bytes::BytesMut};

#[pin_project]
#[derive(Clone)]
pub struct SyncBus<R: AsyncRead, W: AsyncWrite>(#[pin] Arc<Mutex<MessageBus<R, W>>>);

impl<R, W> SyncBus<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> SyncBus<R, W> {
        SyncBus(Arc::new(Mutex::new(MessageBus::new(reader, writer))))
    }

    pub fn pin(&self) -> Pin<&Self> {
        Pin::new(self)
    }

    pub async fn tx(self: Pin<&Self>, msg: HpgMessage) -> Result<(), HpgRemoteError> {
        let this = self.project_ref();
        let bus = &mut *this.0.lock().unwrap();

        Pin::new(bus).tx(msg).await?;
        Ok(())
    }

    pub async fn rx(self: Pin<&Self>) -> Result<Option<HpgMessage>, HpgRemoteError> {
        let this = self.project_ref();
        let bus = &mut *this.0.lock().unwrap();

        Ok(Pin::new(bus).rx().await?)
    }
}

#[pin_project]
pub struct MessageBus<R, W> {
    #[pin]
    reader: FramedRead<R, HpgCodec<HpgMessage>>,
    #[pin]
    writer: FramedWrite<W, HpgCodec<HpgMessage>>,
    id: AtomicU64,
}

impl<R, W> MessageBus<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> MessageBus<R, W> {
        let reader = FramedRead::new(reader, HpgCodec::new());
        let writer = FramedWrite::new(writer, HpgCodec::new());

        Self { reader, writer, id: AtomicU64::new(0) }
    }

    async fn write_file(&self, msg: &HpgMessage) {
        let id = self.id.fetch_add(1, Ordering::Relaxed);
        let mut f = File::options().create(true).write(true).open(format!("hpg-{}.dat", id)).await.unwrap();
        let mut codec: HpgCodec<HpgMessage> = HpgCodec::new();
        let mut bytes = BytesMut::new();
        codec.encode(msg.clone(), &mut bytes).unwrap();
        f.write_all(&bytes).await.unwrap();
    }

    pub async fn tx(self: Pin<&mut Self>, msg: HpgMessage) -> Result<(), HpgRemoteError> {
        self.write_file(&msg).await;
        let mut this = self.project();
        this.writer.send(msg).await?;
        Ok(())
    }

    pub async fn rx(self: Pin<&mut Self>) -> Result<Option<HpgMessage>, HpgRemoteError> {
        let mut this = self.project();
        match time::timeout(Duration::from_secs(500), this.reader.next()).await {
            Ok(Some(Ok(m))) => {
                //received message
                return Ok(Some(m));
            }
            Ok(Some(Err(e))) => {
                return Err(e);
            }
            Ok(None) => {
                // Stream closed
                return Ok(None);
            }
            Err(_) => {
                //timeout
                return Err(HpgRemoteError::Unknown("Timed out".to_string()));
            }
        }
    }
}
