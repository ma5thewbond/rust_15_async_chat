use std::{
    process::exit,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result};
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    net::TcpStream,
};

use rust_15_async_chat::async_chat_msg::AsyncChatMsg;

static END_INPUT: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() -> Result<()> {
    // create connection
    let stream = TcpStream::connect("127.0.0.1:11112")
        .await
        .with_context(|| "Connecting to network address failed")?;
    let (mut reader, mut writer) = stream.into_split();

    // handle keyboard input
    let stdin = BufReader::new(stdin());
    let mut lines = stdin.lines();

    println!("Client connected to AsyncChatServer");
    println!("Enter your name and password:");

    // get user name and password and validate against server
    let name = loop {
        let Ok(Some(name)) = lines.next_line().await else {
            eprintln!("Getting username and password failed, quit");
            exit(0);
        };

        match name.split_once(' ') {
            None => {
                println!("Login and password cannot be empty");
                continue;
            }
            Some((login, password)) => {
                if login.trim().len() != 0 && password.len() != 0 {
                    if AsyncChatMsg::login(login.trim().into(), password.into(), &mut writer)
                        .await
                        .is_err()
                    {
                        eprintln!("Sending login failed");
                        exit(0);
                    }

                    if let Ok(server_msg) = AsyncChatMsg::receive(&mut reader).await {
                        println!("{server_msg}");
                        if let AsyncChatMsg::Text(_from, msg) = server_msg {
                            if msg.starts_with("ERROR") {
                                println!("Login failed: {msg}");
                                continue;
                            }
                        }
                        break login.to_string();
                    }
                } else {
                    println!("Login and password cannot be empty! Try again");
                }
            }
        };
    };

    let write_task = tokio::spawn(async move {
        while let Ok(Some(line)) = lines.next_line().await {
            match line.split_once(' ') {
                None => {
                    if AsyncChatMsg::create_text(name.clone(), line.clone())
                        .unwrap()
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
                Some((".image", path)) => {
                    if AsyncChatMsg::create_image(name.clone(), path.into())
                        .await
                        .unwrap()
                        .send(&mut writer)
                        .await
                        .is_err()
                    {
                        eprintln!("Sending message to server failed");
                        continue;
                    }
                }
                Some((".file", path)) => {
                    if AsyncChatMsg::create_file(name.clone(), path.into())
                        .await
                        .unwrap()
                        .send(&mut writer)
                        .await
                        .is_err()
                    {
                        eprintln!("Sending message to server failed");
                        continue;
                    }
                }
                _ => {
                    if AsyncChatMsg::create_text(name.clone(), line.clone())
                        .unwrap()
                        .send(&mut writer)
                        .await
                        .is_err()
                    {
                        eprintln!("Sending message to server failed");
                        continue;
                    }
                }
            }
        }
    });

    let read_task = tokio::spawn(async move {
        loop {
            let msg = match AsyncChatMsg::receive(&mut reader).await {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("Receiving message from server failed with error: {e}");
                    break;
                }
            };
            println!("{}", msg);
            if matches!(msg, AsyncChatMsg::File(_, _, _))
                || matches!(msg, AsyncChatMsg::Image(_, _, _))
            {
                if let Err(e) = msg.store_file().await {
                    eprintln!("Saving incomming file failed with error: {e}");
                };
            }

            if END_INPUT.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    _ = tokio::join!(write_task, read_task);
    Ok(())
}
