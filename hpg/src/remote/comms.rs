use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::error::HpgRemoteError;

use super::{codec::HpgCodec, messages::HpgMessage};
use futures_util::{SinkExt, StreamExt};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time,
};
use tokio_util::codec::{FramedRead, FramedWrite};

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
}

impl<R, W> MessageBus<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> MessageBus<R, W> {
        let reader = FramedRead::new(reader, HpgCodec::new());
        let writer = FramedWrite::new(writer, HpgCodec::new());

        Self { reader, writer }
    }

    pub async fn tx(self: Pin<&mut Self>, msg: HpgMessage) -> Result<(), HpgRemoteError> {
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
