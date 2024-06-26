use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    net::TcpStream,
};

use rust_15_async_chat::async_chat_msg::AsyncChatMsg;

static END_INPUT: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() -> Result<()> {
    let stream = TcpStream::connect("127.0.0.1:11112")
        .await
        .with_context(|| "Connecting to network address failed")?;

    println!("Client connected to AsyncChatServer");

    let stdin = BufReader::new(stdin());

    let mut lines = stdin.lines();

    let (mut reader, mut writer) = stream.into_split();
    let write_task = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            if AsyncChatMsg::Text("user".to_string(), line.clone())
                .send(&mut writer)
                .await
                .is_err()
            {
                eprintln!("Sending message to server failed");
                continue;
            }
            if line == ".quit" {
                END_INPUT
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |_| Some(true))
                    .unwrap();
                break;
            }
        }
    });

    let read_task = tokio::spawn(async move {
        loop {
            let msg = match AsyncChatMsg::receive(&mut reader).await {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("Receiving message from server failed with error: {e}");
                    continue;
                }
            };
            println!("{}", msg);
            if END_INPUT.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    _ = tokio::join!(write_task, read_task);
    Ok(())
}
