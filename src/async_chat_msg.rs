//! contains custom message enum and implementation for it

use core::fmt;

use anyhow::{bail, Context, Result};
use chrono::prelude::*;
use nanodb::nanodb::NanoDB;
use serde_derive::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// custom message enum to hold message data
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AsyncChatMsg {
    /// simplest text message variant, contains username from who the message is and text of the message
    Text(String, String), // from, message
    /// message containing any file data, contains username from who the message is, name of the file and data of the file
    File(String, String, Vec<u8>), // from, filename, file data
    /// message containing image, contains username from who the message is, filename of the image and data of the image
    Image(String, String, Vec<u8>), // from, filename, file data
    /// special login message containing user name and password for user to login
    Login(String, String), // login, password
}

use crate::AsyncChatMsg::{File as AsyncMsgFile, Image as AsyncMsgImage};
use crate::{
    deserialize_msg, ensure_folder, get_file_data, get_file_name, save_msg_to_db, serialize_msg,
};

impl AsyncChatMsg {
    /// creates text variant of message from provided parameters
    pub fn create_text(from: String, msg: String) -> Result<AsyncChatMsg> {
        let m = AsyncChatMsg::Text(from, msg);
        return Ok(m);
    }

    /// creates file variant of message from provided parameters
    pub async fn create_file(from: String, path: String) -> Result<AsyncChatMsg> {
        let file_name = get_file_name(&path);
        let data: Vec<u8> = get_file_data(&path)
            .await
            .with_context(|| "Getting data for file failed")?;
        let m = AsyncChatMsg::File(from, file_name, data);
        return Ok(m);
    }

    /// creates image variant of message from provided parameters
    pub async fn create_image(from: String, path: String) -> Result<AsyncChatMsg> {
        let file_name = get_file_name(&path);
        let data: Vec<u8> = get_file_data(&path)
            .await
            .with_context(|| "Getting data for image failed")?;
        let m = AsyncChatMsg::Image(from, file_name, data);
        return Ok(m);
    }

    /// send message over tcp stream to server and return result
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

    /// receive message from the provided stream
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

    /// create login message and send it to server
    pub async fn login<T: AsyncWriteExt + Unpin>(
        login: String,
        password: String,
        stream: &mut T,
    ) -> Result<()> {
        let m = AsyncChatMsg::Login(login, password);
        m.send(stream).await
    }

    /// store file to the filesystem, depending on message type either store file in the ./files folder or image in ./images, folders are created if doesn't exists
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

    /// save message to db, data of the files are not stored
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
            _ => return Ok(()), // do not save Login or other types to db
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

    ///get text from the message, in case of file and image, return filename, in case of login message return login
    pub fn get_text(&self) -> &str {
        let text = match self {
            AsyncChatMsg::Text(_, msg) => msg,
            AsyncChatMsg::Image(_, filename, _) => filename,
            AsyncChatMsg::File(_, filename, _) => filename,
            AsyncChatMsg::Login(login, _) => login,
        };
        return text;
    }
}

/// lightweight version of AsyncChatMsg for storing in db, doesn't contain data of the files
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AsyncChatMsgDB {
    /// simplest text message variant, contains username from who the message is and text of the message
    Text(String, String), // from, message
    /// file message variant, contains username from who the message is, file name and file data
    File(String, String), // from, filename
    /// image message variant, contains username from who the message is, image name and file data
    Image(String, String), // from, filename
}

/// implementation of Display trait, so AsyncChatMessage can be easily displayed on console
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
            AsyncChatMsg::Login(login, _password) => format!("{login}: ********* (you didn't really think that I would print password here, did you?"),
        };
        write!(f, "{}", printable)
    }
}
