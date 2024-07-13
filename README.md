# rust_15_async_chat
New, significantly improved async version of client/server chat application, using DB for storage. Rewritten from scratch.

Using Tokio for asynchronous execution
Using NanoDB for message storage to DB in json format, saving to local file in format {"timestamp|name" => AsyncChatMsgDB }

AsyncChatDB is simplified object without data for image and file messages, so only name of the file is stored in history

When client starts, user is asked for name and password. Then in the loop following is tested until successfull login
1. Name and password cannot be empty string (used trim().len() != 0 for validation), if empty name or password provided, user is repeatedly promted to enter new one until they are not empty
2. Name and password are sent to server for validation. On first login attemt, user is created in servers db file, on every other attempt, password for the user is loaded from db and compared with one sent.
If the password doesn't match, error message is sent back to user and user can try another login/password combination.
3. Once successfully validated, username is checked in existing clients. If other user with same name is already in the chat, error is sent back to user that he has to use different user name or disconnect from other session

to run tests in /tests folder execute
> cargo test