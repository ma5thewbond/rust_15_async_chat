use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, broadcast::Sender, RwLock},
};

use rust_15_async_chat::async_chat_msg::AsyncChatMsg;

#[tokio::main]
async fn main() -> Result<()> {
    let server = TcpListener::bind("0.0.0.0:11112")
        .await
        .with_context(|| "Connecting to network address failed")?;

    println!("AsyncChatServer is running");

    let clients: Arc<RwLock<HashMap<SocketAddr, TcpStream>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let (br_send, _br_recv) = broadcast::channel(1024);
    let mut waiting = true;
    loop {
        let client_count = clients.read().await.len();
        if !waiting && client_count == 0 {
            break;
        }
        waiting = false;

        let Ok((stream, addr)) = server.accept().await else {
            eprintln!("couldn't get client");
            continue;
        };

        let sender = br_send.clone();
        let mut receiver = br_send.subscribe();

        let (mut stream_reader, mut stream_writer) = stream.into_split();
        let clients_copy = clients.clone();

        tokio::spawn(async move {
            loop {
                println!("Receiving msg from client");
                match AsyncChatMsg::receive(&mut stream_reader).await {
                    Ok(ref msg @ AsyncChatMsg::Text(ref from, ref text)) => {
                        println!("{from}: {text}");
                        if sender.send((msg.clone(), addr)).is_err() {
                            break;
                        }
                    }
                    Ok(ref msg @ AsyncChatMsg::Image(ref from, ref text, ref data)) => {
                        println!("{from}: {text} ({})", data.len());
                        if sender.send((msg.clone(), addr)).is_err() {
                            break;
                        }
                    }
                    Ok(ref msg @ AsyncChatMsg::File(ref from, ref text, ref data)) => {
                        println!("{from}: {text} ({})", data.len());
                        if sender.send((msg.clone(), addr)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("error receiving message from client: {e}");
                        clients_copy.write().await.remove_entry(&addr);
                        if clients_copy.read().await.len() == 0 {
                            send_quit_ping(sender).await;
                        }
                        break;
                    }
                };
                println!("msg received");
            }
        });

        tokio::spawn(async move {
            while let Ok((msg, other_addr)) = receiver.recv().await {
                if other_addr == addr {
                    continue;
                }

                println!("Sending broadcast message {msg}");
                if let Err(e) = msg.send(&mut stream_writer).await {
                    eprintln!("error sending broadcast message with error: {e}");
                    break;
                }
            }
        });
    }

    return Ok(());
}

async fn send_quit_ping(_send_msg: Sender<(AsyncChatMsg, SocketAddr)>) {
    TcpStream::connect(format!("127.0.0.1:11111"))
        .await
        .unwrap();
    // send_msg
    //     .send((
    //         Some(SocketAddr::from_str(&format!("127.0.0.1:{}", self.port)).unwrap()),
    //         MyNetMsg::quit_msq(String::from(".quit")),
    //     ))
    //     .unwrap();
}
