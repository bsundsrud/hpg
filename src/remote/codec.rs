use std::{marker::PhantomData, mem::size_of};

use crate::error::HpgRemoteError;

use serde::{de::DeserializeOwned, Serialize};
use tokio_util::{
    bytes::{Buf, BytesMut},
    codec::{Decoder, Encoder},
};

const HEADER_SIZE: usize = size_of::<u64>();

pub struct HpgCodec<T> {
    _data: PhantomData<T>,
}

impl<T> HpgCodec<T> {
    pub fn new() -> HpgCodec<T> {
        HpgCodec {
            _data: PhantomData,
        }
    }
}

impl<T> Decoder for HpgCodec<T>
where
    T: DeserializeOwned,
{
    type Item = T;
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

impl<T> Encoder<T> for HpgCodec<T>
where
    T: Serialize,
{
    type Error = HpgRemoteError;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
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
