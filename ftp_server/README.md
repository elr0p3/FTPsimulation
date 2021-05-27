## FTP server done in Rust

This FTP implementation is done in Rust. It's just a proof of concept and an university work
so don't expect real use cases for this!

### Implementation

- Before doing anything you should send the following to the server authentification to the server

```
** NOTE: <endline> == \r\n
USER <username> <endline>
```

- and...

```
PASS <password> <endline>
```

- Note that the password being sent is not encrypted (be careful!).

- Then you can do anything you want! Basically we provide you the following commands: (Note
  that before using store, list and retr, you must open a data channel with PORT or PASV, see ftp protocol for more details).

```
-- Stores a file on the desired path, will return an error if the path doesn't exist.
STOR <path> <endline>
```

```
-- Sends the desired file, will return an error if the path doesn't exist
RECV <path> <endline>
```

```
-- Returns in a LS format the directories in the path.
LIST <path> <endline>
```

```
-- Will connect to that IP address for a data transfer
PORT <h0>,<h1>,<h2>,<h3>,<p0>,<p1><endline>
```

```
-- Will send you an IP address that you must connect to for receiving/sending data
PASV<endline>
```

```
-- Returns the current path
PWD<endline>
```

```
-- Goes to the specified path
CWD <path><endline>
```

```
-- Creates the specified folder
MKD <path><endline>
```

```
-- Removes the specified folder (recursive!!! Be careful!)
RMD <path><endline>
```

```
-- Removes the specified file
DELE <path><endline>
```

```
-- Specifies the folder/file to rename/move
RNFR <path><endline>
```

```
-- Specifies the new path and name to the folder/file
RNTO <path><endline>
```

```
-- Quits the command connection
QUIT<endline>
```

## Running

We are building the project with the builtin package manager for Rust `cargo`

```
>> cargo run --release

FTP Server 1.0
Gabriel Villalonga @gabivlj
Rodrigo Pereira @_
Daniel Gracia @DaniGMX
Simple to use FTP server, with multithreading and non-blocking behaviour in mind for maximum concurrent file transfers
for multiple users.

USAGE:
    ftp_server [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --capacity <CAPACITY>    Sets maximum concurrent connections [default: 500]
    -p, --port <PORT>            Set port [default: 8080]
```
