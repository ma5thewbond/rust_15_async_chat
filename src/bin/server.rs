use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use nanodb::nanodb::NanoDB;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, RwLock},
};

use rust_15_async_chat::PORT;
use rust_15_async_chat::{async_chat_msg::AsyncChatMsg, validate_user_in_db};

#[tokio::main]
async fn main() -> Result<()> {
    let server = TcpListener::bind(format!("0.0.0.0:{PORT}"))
        .await
        .with_context(|| "Connecting to network address failed")?;

    println!("AsyncChatServer is running");

    let clients: Arc<RwLock<HashMap<String, SocketAddr>>> = Arc::new(RwLock::new(HashMap::new()));

    let (br_send, _br_recv) = broadcast::channel(1024);
    let mut waiting = true;
    let chat_db = NanoDB::open("chatdb.json")
        .unwrap_or_else(|e| panic!("Opening db file chatdb.json failed {}", e));
    let users_db = NanoDB::open("userdb.json")
        .unwrap_or_else(|e| panic!("Opening db file userdb.json failed {}", e));

    // handle client
    'client: loop {
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

        // validate user login, if failed, try again
        let name = loop {
            let Ok(AsyncChatMsg::Login(name, password)) =
                AsyncChatMsg::receive(&mut stream_reader).await
            else {
                eprintln!("Login from the client not received");
                continue 'client;
            };

            match validate_user_in_db(&name, &password, users_db.clone()).await {
                Ok(false) => {
                    let wrong_pass_msg = AsyncChatMsg::create_text(
                        "Server".into(),
                        format!("ERROR: Incorrect password for login {name}"),
                    )
                    .unwrap();
                    if let Err(e) = wrong_pass_msg.send(&mut stream_writer).await {
                        eprintln!("Sending wrong password failed with error {e}");
                    }
                    continue;
                }
                Ok(true) => (),
                Err(error) => {
                    eprintln!("Validation of user {name} failed with error: {error}");
                    continue;
                }
            }

            if clients.read().await.contains_key(&name) {
                let name_used_msg = AsyncChatMsg::create_text(
                    "Server".into(),
                    format!("ERROR: User {name} is already logged in, please choose another or disconnect from existing session"),
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
                break name;
            }
        };
        println!("User {name} has connected");

        // _ = sender.send((
        //     AsyncChatMsg::create_text("Server".to_string(), format!("User {name} has connected"))
        //         .unwrap(),
        //     SocketAddr::from_str(&format!("127.0.0.1:{PORT}")).unwrap(),
        // ));

        clients.write().await.insert(name.clone(), addr);
        let clients_copy = clients.clone();

        tokio::spawn({
            let db = chat_db.clone();
            async move {
                loop {
                    let message = AsyncChatMsg::receive(&mut stream_reader).await;
                    match message {
                        Ok(ref msg @ AsyncChatMsg::Text(ref _from, ref text)) => {
                            println!("{msg}");
                            if text == ".quit" {
                                // if last client disconnected, then send quit ping to self to break the loops
                                clients_copy.write().await.remove_entry(&name);
                                if clients_copy.read().await.len() == 0 {
                                    let _ = send_quit_ping() //sender, &addr, name
                                        .await
                                        .with_context(|| "Sending disconnect message failed (1)");
                                }
                            }
                        }
                        Ok(ref msg @ AsyncChatMsg::Image(ref _from, ref _text, ref _data)) => {
                            println!("{msg}");
                        }
                        Ok(ref msg @ AsyncChatMsg::File(ref _from, ref _text, ref _data)) => {
                            println!("{msg}");
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
                        _ => (),
                    };
                    let message = message.unwrap();
                    // send quit message with disconnect info for everyone
                    if sender.send((message.clone(), addr)).is_err() {
                        eprintln!("Sending message to broadcast failed");
                    }
                    if let Err(e) = message.save_to_db(db.clone()).await {
                        eprintln!("Saving msg to db failed with error: {e}");
                    }
                    if message.get_text() == ".quit" {
                        break;
                    }
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
