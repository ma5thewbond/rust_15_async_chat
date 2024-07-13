//! Library with common functions for client and server
#![warn(missing_docs)]
use anyhow::{Context, Result};
use async_chat_msg::{AsyncChatMsg, AsyncChatMsgDB};
use nanodb::{error::NanoDBError, nanodb::NanoDB};
use std::{io::Error, path::Path};
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};

/// reference async_chat_msg file
pub mod async_chat_msg;
/// define port to which client and server are connected
pub const PORT: &str = "11112";

/// serialize message to binary vec for sending via network
pub fn serialize_msg(msg: &AsyncChatMsg) -> Result<Vec<u8>> {
    return Ok(serde_cbor::to_vec(&msg).with_context(|| "Serialization of message failed")?);
}

/// deserialize message from vector to object
pub fn deserialize_msg(data: Vec<u8>) -> Result<AsyncChatMsg> {
    return Ok(serde_cbor::from_slice(&data).with_context(|| "Deserialization of message failed")?);
}

/// get file name from path provided
pub fn get_file_name(path: &str) -> String {
    let path: &Path = Path::new(path.trim());
    return path.file_name().unwrap().to_str().unwrap().to_string();
}

/// load data from filesystem from path provided
pub async fn get_file_data(path: &str) -> Result<Vec<u8>> {
    let path = Path::new(path.trim());
    let mut f = File::open(path).await?;
    let metadata = fs::metadata(path).await?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).await?;

    return Ok(buffer);
}

/// make sure that the folder provided as path parameter exists and is a folder
pub async fn ensure_folder(path: &str) -> Result<()> {
    let path = Path::new(path.trim());
    if !path.exists() {
        fs::create_dir_all(path).await?;
        ()
    }

    let meta = fs::metadata(path).await?;
    if path.exists() && meta.is_file() {
        return Err(anyhow::Error::new(Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Path already exists as a file",
        )));
    }

    Ok(())
}

/// saves message to databaze
pub async fn save_msg_to_db(timestamp: String, msg: AsyncChatMsgDB, mut db: NanoDB) -> Result<()> {
    let from = match msg.clone() {
        AsyncChatMsgDB::Text(from, _) => from,
        AsyncChatMsgDB::Image(from, _) => from,
        AsyncChatMsgDB::File(from, _) => from,
    };
    db.insert(&(timestamp + "|" + &from), msg).await?;
    if let Err(e) = db.write().await {
        eprintln!("Saving db to file failed with error {e}");
    }
    Ok(())
}

/// validate user name and password against data stored in db, if user doesn't exist yet, create it
pub async fn validate_user_in_db(login: &str, password: &str, mut db: NanoDB) -> Result<bool> {
    let pass = get_password_for_user(login, &db).await;
    match pass {
        Ok(pass) => {
            println!("Password for user {login} in db is '{password}'");
            if pass != password {
                // TODO md5
                return Ok(false);
            }
            return Ok(true);
        }
        Err(NanoDBError::KeyNotFound(error)) => {
            eprintln!("Error getting password for user {login} from db, key not found: {error}");
            // this is new user, so save him and return true
            db.insert(login, password).await?;
            if let Err(e) = db.write().await {
                eprintln!("Saving db to file failed with error {e}");
            }
            return Ok(true);
        }
        Err(error) => {
            eprintln!("Error getting password for user {login} from db: {error}");
            return Ok(false);
        }
    }
}

/// get password for user name provided as parameter
pub async fn get_password_for_user(login: &str, db: &NanoDB) -> Result<String, NanoDBError> {
    let pass = db.data().await.get(&login)?.into()?;
    return Ok(pass);
}
