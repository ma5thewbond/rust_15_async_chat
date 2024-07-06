use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use nanodb::nanodb::NanoDB;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, RwLock},
};

use rust_15_async_chat::async_chat_msg::AsyncChatMsg;
use rust_15_async_chat::PORT;

#[tokio::main]
async fn main() -> Result<()> {
    let server = TcpListener::bind(format!("0.0.0.0:{PORT}"))
        .await
        .with_context(|| "Connecting to network address failed")?;

    println!("AsyncChatServer is running");

    let clients: Arc<RwLock<HashMap<String, SocketAddr>>> = Arc::new(RwLock::new(HashMap::new()));

    let (br_send, _br_recv) = broadcast::channel(1024);
    let mut waiting = true;
    let db = NanoDB::open("msgsdb.json")
        .unwrap_or_else(|e| panic!("Opening db file msgsdb.json failed {}", e));

    loop {
        let Ok((stream, addr)) = server.accept().await else {
            eprintln!("couldn't get client");
            continue;
        };

        let client_count = clients.read().await.len();
        if !waiting && client_count == 0 {
            println!("No more clients, quit");
            break;
        }
        waiting = false;

        let sender = br_send.clone();
        let mut receiver = br_send.subscribe();

        let (mut stream_reader, mut stream_writer) = stream.into_split();

        let Ok(AsyncChatMsg::Text(name, _connected_msg)) =
            AsyncChatMsg::receive(&mut stream_reader).await
        else {
            eprintln!("Name from the client not received");
            continue;
        };

        if clients.read().await.contains_key(&name) {
            let name_used_msg = AsyncChatMsg::create_text(
                "Server".into(),
                format!("ERROR: Name {name} is already used, please choose another"),
            )
            .unwrap();
            if let Err(e) = name_used_msg.send(&mut stream_writer).await {
                eprintln!("Sending existing name warning failed with error {e}");
            }
            continue;
        } else {
            let welcome_msg = AsyncChatMsg::create_text(
                "Server".into(),
                format!("{name}, welcome on the AsyncChatServer!"),
            )
            .unwrap();
            if let Err(e) = welcome_msg.send(&mut stream_writer).await {
                eprintln!("Sending welcome message failed with error {e}");
            }
        }

        println!("User {name} has connected");

        // _ = sender.send((
        //     AsyncChatMsg::create_text("Server".to_string(), format!("User {name} has connected"))
        //         .unwrap(),
        //     SocketAddr::from_str(&format!("127.0.0.1:{PORT}")).unwrap(),
        // ));

        clients.write().await.insert(name.clone(), addr);
        let clients_copy = clients.clone();

        tokio::spawn({
            let db = db.clone();
            async move {
                loop {
                    match AsyncChatMsg::receive(&mut stream_reader).await {
                        Ok(ref msg @ AsyncChatMsg::Text(ref _from, ref text)) => {
                            println!("{msg}");
                            if text == ".quit" {
                                // send quit message with disconnect info for everyone
                                if sender.send((msg.clone(), addr)).is_err() {
                                    eprintln!("Sending message to broadcast failed");
                                }
                                // if last client disconnected, then send quit ping to self to break the loops
                                clients_copy.write().await.remove_entry(&name);
                                if clients_copy.read().await.len() == 0 {
                                    let _ = send_quit_ping() //sender, &addr, name
                                        .await
                                        .with_context(|| "Sending disconnect message failed (1)");
                                }
                                if let Err(e) = msg.save_to_db(db.clone()).await {
                                    eprintln!("Saving msg to db failed with error: {e}");
                                }
                                break;
                            }
                            if sender.send((msg.clone(), addr)).is_err() {
                                eprintln!("Sending message to broadcast failed");
                            }
                            if let Err(e) = msg.save_to_db(db.clone()).await {
                                eprintln!("Saving msg to db failed with error: {e}");
                            }
                        }
                        Ok(ref msg @ AsyncChatMsg::Image(ref _from, ref _text, ref _data)) => {
                            println!("{msg}");
                            if sender.send((msg.clone(), addr)).is_err() {
                                eprintln!("Sending message to broadcast failed");
                            }
                            if let Err(e) = msg.save_to_db(db.clone()).await {
                                eprintln!("Saving msg to db failed with error: {e}");
                            }
                        }
                        Ok(ref msg @ AsyncChatMsg::File(ref _from, ref _text, ref _data)) => {
                            println!("{msg}");
                            if sender.send((msg.clone(), addr)).is_err() {
                                eprintln!("Sending message to broadcast failed");
                            }
                            if let Err(e) = msg.save_to_db(db.clone()).await {
                                eprintln!("Saving msg to db failed with error: {e}");
                            }
                        }
                        Err(e) => {
                            eprintln!("error receiving message from client: {e}");
                            //let name = clients_copy.read().await.get(&addr).unwrap().clone();
                            clients_copy.write().await.remove_entry(&name);
                            if clients_copy.read().await.len() == 0 {
                                let _ = send_quit_ping() //sender, &addr, name
                                    .await
                                    .with_context(|| "Sending disconnect message failed (1)");
                            }
                            break;
                        }
                    };
                }
            }
        });

        // handle sending broadcast messages
        tokio::spawn(async move {
            while let Ok((msg, other_addr)) = receiver.recv().await {
                match msg.clone() {
                    AsyncChatMsg::Text(from, text) => {
                        if text == ".quit" {
                            let bye_msg = AsyncChatMsg::Text(
                                "Server".to_string(),
                                format!("User {from} has disconnected"),
                            );
                            let _ = bye_msg
                                .send(&mut stream_writer)
                                .await
                                .with_context(|| "Sending disconnect message failed (2)");
                            // if current client sent quit message, break the while and exit the thread
                            if other_addr == addr {
                                break;
                            }
                        } else {
                            if other_addr != addr {
                                if let Err(e) = msg.send(&mut stream_writer).await {
                                    eprintln!("error sending broadcast message with error: {e}");
                                    break;
                                }
                            }
                        }
                        // if this is quit ping message from server, break the loop and close server
                        if from == "Server" {
                            break;
                        }
                    }
                    // broadcast other types of messages to everyone except my self
                    _ => {
                        if other_addr != addr {
                            if let Err(e) = msg.send(&mut stream_writer).await {
                                eprintln!("error sending broadcast message with error: {e}");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    //tokio::join!(_client_handle, _broadcast_handle); //(client_handle, broadcast_handle);

    return Ok(());
}

async fn send_quit_ping() -> Result<()> {
    TcpStream::connect(format!("127.0.0.1:{PORT}"))
        .await
        .with_context(|| "Connection to server failed")?;

    Ok(())
}
