# P2P File Transfer and Messaging CLI Application

## Project Description

A command-line application (CLI) that enables peer-to-peer (P2P) messaging and file transfer between users on the same local network. Inspired by [sharedrop.io](https://sharedrop.io), this application allows users to discover available devices, establish secure TCP connections, and transfer data without the need for external servers.

This project is implemented as part of the Rust course at the University of Warsaw (MIMUW).

## Controls

### Peer List
- `'Esc'`: Exit the application.  
- `⬆️` / `⬇️`: Navigate up and down in the list.  
- `'Enter'`: Open the editor view for the selected peer conversation.  

### Editor
- `'Esc'`: Go back to the peer list view.  
- `'Tab'`: Toggle between text-sending mode and file-sending mode.  
- `'Enter'` + Modifier (*`'Alt'`, `'Shift'`, `'Ctrl'`, etc.*): Insert a new line. *Warning* you should use modifier that is not binded by your environment to some other action so that application can detect that event.
- `'Enter'`: Send the composed message or file.  
- `⬆️`: Switch to the message list view.  
- Other keys: Work like in standard text editors (keyboard-wise, no mouse action is being detected currently).  

### Message List
- `'Enter'` on a text message: Copy the message content to the clipboard.  
- `'Enter'` on a peer's file message: Download the file to the system's default download folder.  
- `⬆️` / `⬇️`: Navigate up and down in the list.  
- `'Esc'`: Go back to the editor view. 

## Roadmap

### Iteration 1 (*2024-12-12*)
- [x] **Terminal UI**: Design a user-friendly command-line interface.
- [x] **Device Discovery**: Detect available devices on the local network.
- [x] **TCP Connection**: Establish TCP connections with detected devices.
- [x] **Messaging**: Allow users to send messages to other connected users.

### Iteration 2 (*2025-01-09*)
- [x] **File Transfer**: Enable file transfers between devices.
- [ ] **Connection Encryption**: Add encryption to secure connections.
- [x] **Improvement**: Enhance features from the first iteration.

## Team Members
- Jakub Bartecki
