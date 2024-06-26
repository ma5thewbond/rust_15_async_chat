use core::fmt;

use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AsyncChatMsg {
    Text(String, String),
    File(String, String, Vec<u8>),
    Image(String, String, Vec<u8>),
}

impl AsyncChatMsg {
    pub fn create_text(from: String, msg: String) -> Result<AsyncChatMsg> {
        let m = AsyncChatMsg::Text(from, msg);
        return Ok(m);
    }

    pub fn create_file(from: String, msg: String, data: Vec<u8>) -> Result<AsyncChatMsg> {
        let m = AsyncChatMsg::File(from, msg, data);
        return Ok(m);
    }

    pub fn create_image(from: String, msg: String, data: Vec<u8>) -> Result<AsyncChatMsg> {
        let m = AsyncChatMsg::Image(from, msg, data);
        return Ok(m);
    }

    pub async fn send<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        let msg: Vec<u8> = serde_cbor::to_vec(&self).with_context(|| "Encoding message failed")?;
        stream
            .write_all(&(msg.len() as u32).to_be_bytes())
            .await
            .with_context(|| "Sending message size failed")?;
        stream
            .write_all(&msg)
            .await
            .with_context(|| "Sending message failed")?;
        return Ok(());
    }

    pub async fn receive<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<Self> {
        let mut length_bytes = [0; 4];

        stream
            .read_exact(&mut length_bytes)
            .await
            .with_context(|| "Failed to read length")?;

        let length = u32::from_be_bytes(length_bytes);

        let mut msgdata = vec![0; length as usize];
        stream
            .read_exact(&mut msgdata)
            .await
            .with_context(|| "Reading message failed")?;

        let msg: AsyncChatMsg = serde_cbor::from_slice(&msgdata)
            .with_context(|| "Deserialization of message failed")?;

        return Ok(msg);
    }
}

impl fmt::Display for AsyncChatMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            AsyncChatMsg::Text(from, text) => format!("\n{from}: {text}"),
            AsyncChatMsg::File(from, text, data) => {
                format!("\n{}: incomming file {} ({}B)", from, text, data.len())
            }
            AsyncChatMsg::Image(from, text, data) => {
                format!("\n{}: incomming image {} ({}B)", from, text, data.len())
            }
        };
        write!(f, "{}", printable)
    }
}
