# rust_15_async_chat
New, significantly improved async version of client/server chat application, using DB for storage. Rewritten from scratch.

Using Tokio for asynchronous execution
Using NanoDB for message storage to DB in json format, saving to local file in format {"timestamp|name" => AsyncChatMsgDB }

AsyncChatDB is simplified object without data for image and file messages, so only name of the file is stored in history

When client starts, user is asked for name. There are two validations on name:
1. Name cannot be empty string (used trim().len() != 0 for validation), if empty name provided, user is repeatedly promted to enter new one until the name is not empty
2. Name cannot be same as other user already in the chat, in this case client is shut down and has to be started again.
