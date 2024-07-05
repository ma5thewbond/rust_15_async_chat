use std::{io::Error, path::Path};

use anyhow::{Context, Result};
use async_chat_msg::AsyncChatMsg;
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};

pub mod async_chat_msg;
pub const PORT: &str = "11112";

pub fn serialize_msg(msg: &AsyncChatMsg) -> Result<Vec<u8>> {
    return Ok(serde_cbor::to_vec(&msg).with_context(|| "Serialization of message failed")?);
}

pub fn deserialize_msg(data: Vec<u8>) -> Result<AsyncChatMsg> {
    return Ok(serde_cbor::from_slice(&data).with_context(|| "Deserialization of message failed")?);
}

pub fn get_file_name(path: &str) -> String {
    let path: &Path = Path::new(path.trim());
    return path.file_name().unwrap().to_str().unwrap().to_string();
}

pub async fn get_file_data(path: &str) -> Result<Vec<u8>> {
    let path = Path::new(path.trim());
    let mut f = File::open(path).await?;
    let metadata = fs::metadata(path).await?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).await?;

    return Ok(buffer);
}

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
