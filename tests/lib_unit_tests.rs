use std::path::Path;

use nanodb::nanodb::NanoDB;
use rust_15_async_chat::async_chat_msg::AsyncChatMsg;
use rust_15_async_chat::*;
use tokio::fs::{remove_dir, remove_file};

#[test]
fn message_serialize_is_ok() {
    // prepare
    let msg = AsyncChatMsg::Text("martin".into(), "hello".into());
    // act
    let serialized = serialize_msg(&msg);
    // assert
    assert!(serialized.is_ok());
}

#[test]
fn message_deserialize_is_ok() {
    // prepare
    let msg = AsyncChatMsg::Text("martin".into(), "hello".into());
    // act
    let serialized = serialize_msg(&msg).unwrap();
    let deserialized = deserialize_msg(serialized);
    // assert
    assert!(deserialized.is_ok());
    assert_eq!(deserialized.unwrap().get_text(), msg.get_text());
}

#[tokio::test]
async fn ensure_folder_exists() {
    // prepare
    let folder_name = "testfolder";
    // act
    let mut folder_result = ensure_folder(&folder_name).await;
    // assert
    assert!(folder_result.is_ok());
    assert!(Path::new(folder_name).exists());
    folder_result = ensure_folder(&folder_name).await;
    assert!(folder_result.is_ok());
    assert!(Path::new(folder_name).exists());
    // cleanup
    _ = remove_dir(folder_name).await;
    assert!(!Path::new(folder_name).exists());
}

#[tokio::test]
async fn save_message_to_db_message_saved() {
    // prepare
    let testfile = "testdb.json";
    let db = NanoDB::open(testfile).unwrap();
    let msg = AsyncChatMsg::Text("martin".into(), "hello".into());
    // act
    let dbres = msg.save_to_db(db).await;
    // assert
    assert!(dbres.is_ok());
    assert!(Path::new(testfile).exists());
    // cleanup
    _ = remove_file(testfile).await;
    assert!(!Path::new(testfile).exists());
}

#[tokio::test]
async fn store_message_file_file_stored() {
    let filename = "test.zip";
    let msg = AsyncChatMsg::create_file("martin".into(), filename.into())
        .await
        .unwrap();
    // act
    let res = msg.store_file().await;
    // assert
    assert!(res.is_ok());
    assert!(Path::new(&format!("files/{filename}")).exists());
    // cleanup
    _ = remove_file(&format!("files/{filename}")).await;
    _ = remove_dir("files").await;
    assert!(!Path::new("files").exists());
}

#[tokio::test]
async fn validate_user_in_db_new_user_created() {
    // prepare
    let testfile = "testuserdb.json";
    let db = NanoDB::open(testfile).unwrap();
    // act
    let dbres = validate_user_in_db("martin", "password", db).await;
    // assert
    assert!(dbres.is_ok());
    assert!(Path::new(testfile).exists());
    assert_eq!(dbres.unwrap(), true);
    // cleanup
    _ = remove_file(testfile).await;
    assert!(!Path::new(testfile).exists());
}

#[tokio::test]
async fn validate_user_in_db_existing_user_correct_password() {
    // prepare
    let testfile = "testuser2db.json";
    let db = NanoDB::open(testfile).unwrap();
    // act
    let _ = validate_user_in_db("martin", "password", db.clone()).await;
    let dbres = validate_user_in_db("martin", "password", db).await;
    // assert
    assert!(dbres.is_ok());
    assert!(Path::new(testfile).exists());
    assert_eq!(dbres.unwrap(), true);
    // cleanup
    _ = remove_file(testfile).await;
    assert!(!Path::new(testfile).exists());
}

#[tokio::test]
async fn validate_user_in_db_existing_user_incorrect_password() {
    // prepare
    let testfile = "testuser3db.json";
    let db = NanoDB::open(testfile).unwrap();
    // act
    let _ = validate_user_in_db("martin", "password", db.clone()).await;
    let dbres = validate_user_in_db("martin", "password2", db).await;
    // assert
    assert!(dbres.is_ok());
    assert!(Path::new(testfile).exists());
    assert_eq!(dbres.unwrap(), false);
    // cleanup
    _ = remove_file(testfile).await;
    assert!(!Path::new(testfile).exists());
}
