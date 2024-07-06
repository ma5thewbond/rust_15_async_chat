use core::fmt;

use anyhow::{bail, Context, Result};
use chrono::prelude::*;
use nanodb::nanodb::NanoDB;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AsyncChatMsg {
    Text(String, String),           // from, message
    File(String, String, Vec<u8>),  // from, filename, file data
    Image(String, String, Vec<u8>), // from, filename, file data
}

use crate::AsyncChatMsg::{File as AsyncMsgFile, Image as AsyncMsgImage};
use crate::{
    deserialize_msg, ensure_folder, get_file_data, get_file_name, save_msg_to_db, serialize_msg,
};

impl AsyncChatMsg {
    pub fn create_text(from: String, msg: String) -> Result<AsyncChatMsg> {
        let m = AsyncChatMsg::Text(from, msg);
        return Ok(m);
    }

    pub async fn create_file(from: String, path: String) -> Result<AsyncChatMsg> {
        let file_name = get_file_name(&path);
        let data: Vec<u8> = get_file_data(&path)
            .await
            .with_context(|| "Getting data for file failed")?;
        let m = AsyncChatMsg::File(from, file_name, data);
        return Ok(m);
    }

    pub async fn create_image(from: String, path: String) -> Result<AsyncChatMsg> {
        let file_name = get_file_name(&path);
        let data: Vec<u8> = get_file_data(&path)
            .await
            .with_context(|| "Getting data for image failed")?;
        let m = AsyncChatMsg::Image(from, file_name, data);
        return Ok(m);
    }

    pub async fn send<T: AsyncWriteExt + Unpin>(&self, stream: &mut T) -> Result<()> {
        let msg: Vec<u8> = serialize_msg(&self)?;
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

        let msg: AsyncChatMsg = deserialize_msg(msgdata)?;

        return Ok(msg);
    }

    pub async fn store_file(&self) -> Result<()> {
        let (filename, data, path) = match self {
            AsyncMsgImage(_u, filename, data) => {
                ensure_folder("images").await?;
                (filename, data, Path::new("images"))
            }
            AsyncMsgFile(_u, filename, data) => {
                ensure_folder("files").await?;
                (filename, data, Path::new("files"))
            }
            _ => bail!("This is wrong type"),
        };

        // println!(
        //     "Saving file to {}",
        //     path.join(filename).display().to_string()
        // );

        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(path.join(filename))
            .await?;

        f.write(data).await?;
        println!("File {} was saved to {path:?}", filename);
        return Ok(());
    }

    pub async fn save_to_db(&self, db: NanoDB) -> Result<()> {
        let db_msg = match self {
            AsyncChatMsg::Text(from, msg) => {
                AsyncChatMsgDB::Text(from.to_string(), msg.to_string())
            }
            AsyncChatMsg::Image(from, filename, _) => {
                AsyncChatMsgDB::Image(from.to_string(), filename.to_string())
            }
            AsyncChatMsg::File(from, filename, _) => {
                AsyncChatMsgDB::File(from.to_string(), filename.to_string())
            }
        };
        let timestamp: DateTime<Local> = Local::now();
        save_msg_to_db(
            timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            db_msg,
            db,
        )
        .await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AsyncChatMsgDB {
    Text(String, String),  // from, message
    File(String, String),  // from, filename
    Image(String, String), // from, filename
}

impl fmt::Display for AsyncChatMsg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match &self {
            AsyncChatMsg::Text(from, text) => format!("{from}: {text}"),
            AsyncChatMsg::File(from, text, data) => {
                format!("{}: incomming file {} ({}B)", from, text, data.len())
            }
            AsyncChatMsg::Image(from, text, data) => {
                format!("{}: incomming image {} ({}B)", from, text, data.len())
            }
        };
        write!(f, "{}", printable)
    }
}
