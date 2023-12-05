use std::{io::BufWriter, mem::size_of, pin::Pin};

use crate::{
    error::{HpgRemoteError, Result},
    types::Message,
};
use pin_project::pin_project;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::{
    bytes::{Buf, BytesMut},
    codec::{Decoder, Encoder},
};

#[pin_project]
pub struct StreamMessageReader<T>
where
    T: AsyncRead,
{
    #[pin]
    inner: T,
}

impl<T> StreamMessageReader<T>
where
    T: AsyncRead,
{
    pub fn new(reader: T) -> Self {
        Self { inner: reader }
    }

    pub async fn read<V: DeserializeOwned>(self: Pin<&mut Self>) -> Result<V> {
        let mut this = self.project();
        let len = this.inner.read_u64().await?;
        let mut buf = vec![0u8; len.try_into().unwrap()];
        this.inner.read_exact(&mut buf).await?;
        let v = ciborium::from_reader(buf.as_slice())?;
        Ok(v)
    }
}

#[pin_project]
pub struct StreamMessageWriter<T>
where
    T: AsyncWrite,
{
    #[pin]
    inner: T,
}

impl<T> StreamMessageWriter<T>
where
    T: AsyncWrite,
{
    pub fn new(writer: T) -> Self {
        Self { inner: writer }
    }

    pub async fn write<V: Serialize>(self: Pin<&mut Self>, data: &V) -> Result<()> {
        let mut buf: Vec<u8> = Vec::new();
        {
            let mut writer = BufWriter::new(&mut buf);
            ciborium::into_writer(&data, &mut writer)?;
        }
        let len: u64 = buf.len().try_into().unwrap();
        let header = len.to_be_bytes();
        let mut final_buf = Vec::with_capacity(len as usize + header.len());
        final_buf.extend_from_slice(&header);
        final_buf.extend_from_slice(&buf);
        let mut this = self.project();
        this.inner.write_all(&final_buf).await?;
        Ok(())
    }
}

const HEADER_SIZE: usize = size_of::<u64>();

pub struct HpgCodec {}

impl Decoder for HpgCodec {
    type Item = Message;
    type Error = HpgRemoteError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < HEADER_SIZE {
            return Ok(None);
        }
        let mut length_bytes = [0u8; HEADER_SIZE];
        length_bytes.copy_from_slice(&src[..HEADER_SIZE]);
        let length: usize = u64::from_le_bytes(length_bytes).try_into().unwrap();
        if src.len() < HEADER_SIZE + length {
            // waiting for more
            src.reserve(HEADER_SIZE + length - src.len());
            return Ok(None);
        }
        // We now have enough data to read a whole frame, advance the whole window
        let data = src[HEADER_SIZE..HEADER_SIZE + length].to_vec();
        src.advance(HEADER_SIZE + length);

        // Deserialize the message
        match ciborium::from_reader(data.as_slice()) {
            Ok(m) => Ok(Some(m)),
            Err(e) => Err(HpgRemoteError::DeserilizationError(e)),
        }
    }
}

impl Encoder<Message> for HpgCodec {
    type Error = HpgRemoteError;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut bytes: Vec<u8> = Vec::new();
        ciborium::into_writer(&item, &mut bytes)?;
        let length: u64 = bytes.len().try_into().unwrap();
        let length_bytes = length.to_le_bytes();
        dst.reserve(HEADER_SIZE + length as usize);
        dst.extend_from_slice(&length_bytes);
        dst.extend_from_slice(&bytes);
        Ok(())
    }
}
