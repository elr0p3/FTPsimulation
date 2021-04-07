use super::{
    create_response, Action, ActionList, BufferToWrite, FTPServer, HashMutex, RequestContextMutex,
    RequestType, Token,
};
use super::{response::Response, FileTransferType};
use mio::{net::TcpStream, Interest, Waker};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::{io::Error, net::Shutdown};

pub fn close_connection_recursive(
    connection_database: HashMutex<Token, RequestContextMutex>,
    to_delete: Token,
) -> Result<(), Error> {
    let map_conn_arc = connection_database.clone();
    let map_conn = map_conn_arc.lock().unwrap();
    let conn = {
        let connection = map_conn.get(&to_delete);
        if connection.is_none() {
            return Ok(());
        }
        let arc = connection.unwrap().clone();
        arc
    };
    drop(map_conn);
    connection_database.lock().unwrap().remove(&to_delete);
    let mut conn = conn.lock().unwrap();
    println!("[CLOSE_CONNECTION_RECURSIVE] Closing connection recursively");
    match &mut conn.request_type {
        RequestType::FileTransferActive(stream, _, _)
        | RequestType::FileTransferPassive(stream, _, _) => {
            let _ = stream.shutdown(Shutdown::Both)?;
        }
        RequestType::CommandTransfer(stream, _, conn) => {
            // Ignore error to be honest, don't care if we try to close twice
            let _ = stream.shutdown(Shutdown::Both);
            let conn = conn.take();
            if let Some(conn) = &conn {
                close_connection_recursive(map_conn_arc.clone(), *conn)?;
            }
        }
        RequestType::PassiveModePort(_, _) => {}
    }
    Ok(())
}

pub struct HandlerWrite {
    connection_token: Token,

    connection_db: HashMutex<Token, RequestContextMutex>,

    pub actions: Vec<Action>,

    connection: RequestContextMutex,
}

impl HandlerWrite {
    pub fn new(
        connection_token: Token,
        connection_db: HashMutex<Token, RequestContextMutex>,
        connection: RequestContextMutex,
    ) -> Self {
        Self {
            connection_token,
            connection_db,
            actions: Vec::new(),
            connection,
        }
    }

    fn keep_interest(&mut self, waker: &Waker, interest: Interest) -> Result<(), Error> {
        self.actions
            .push((self.connection_token, self.connection.clone(), interest));

        Ok(())
    }

    fn close_connection(&mut self, stream: &mut TcpStream) -> Result<(), Error> {
        stream.shutdown(Shutdown::Both)?;
        let mut map_conn = self.connection_db.lock().unwrap();
        map_conn.remove(&self.connection_token);
        Ok(())
    }

    /// Handles the write request depending on the context of the request.
    /// Also will return a possible callback that needs to be called if it's Ok
    pub fn handle_write(
        &mut self,
        request_type: &mut RequestType,
        waker: &Waker,
    ) -> Result<Option<Box<dyn FnOnce() + Send>>, Error> {
        match request_type {
            RequestType::CommandTransfer(stream, to_write, t) => {
                let maybe_error = stream.flush();
                if let Err(err) = maybe_error {
                    println!("[HANDLE_WRITE] CMD Error flushing the stream: {}", err);
                }
                let written = stream.write(&to_write.buffer[to_write.offset..]);
                if let Ok(written) = written {
                    println!("[HANDLE_WRITE] CMD Writing {} bytes", written);
                    if written + to_write.offset >= to_write.buffer.len() {
                        println!(
                            "[HANDLE_WRITE] - {} - Going back to readable...",
                            self.connection_token.0
                        );
                        to_write.buffer.clear();
                        to_write.offset = 0;
                        self.keep_interest(waker, Interest::READABLE)?;
                        return Ok(to_write.callback_after_sending.take());
                    // if let Some(callback) = to_write.callback_after_sending.take() {
                    //     callback();
                    // }
                    } else {
                        // Keep writing
                        to_write.offset += written;
                        self.keep_interest(waker, Interest::WRITABLE)?;
                    }
                } else if let Err(err) = written {
                    if err.kind() == ErrorKind::WouldBlock {
                        println!(
                            "[HANDLE_WRITE] - {} - Got would block error, keep writing",
                            self.connection_token.0
                        );
                        self.keep_interest(waker, Interest::WRITABLE)?;
                    } else {
                        println!(
                            "[HANDLE_WRITE] - {} - Error writing to socket, closing connection. Error: {}",
                            self.connection_token.0,
                            err
                        );
                        self.close_connection(stream)?;
                        if let Some(t) = t {
                            close_connection_recursive(self.connection_db.clone(), *t)?;
                        }
                    }
                }
            }

            RequestType::FileTransferPassive(stream, ftt, conn_tok) => {
                let _ = stream.flush();
                self.handle_file_transfer(stream, ftt, waker, *conn_tok)?;
            }

            RequestType::FileTransferActive(stream, ftt, conn_tok) => {
                let _ = stream.flush();
                self.handle_file_transfer(stream, ftt, waker, *conn_tok)?;
            }

            _ => return Err(Error::from(ErrorKind::NotFound)),
        }
        Ok(None)
    }

    fn answer_command(&mut self, cmd_connection_token: Token, msg: &str) {
        let mut db = self.connection_db.lock().unwrap();
        let cmd = db.get_mut(&cmd_connection_token);
        if let Some(cmd) = cmd {
            let cmd_arc = cmd.clone();
            let mut cmd = cmd_arc.lock().unwrap();
            if let RequestType::CommandTransfer(_stream, to_write, t) = &mut cmd.request_type {
                t.take();
                to_write.reset(create_response(Response::closing_data_connection(), msg));
                self.actions
                    .push((cmd_connection_token, cmd_arc.clone(), Interest::WRITABLE));
            }
        }
    }

    fn handle_file_transfer(
        &mut self,
        stream: &mut TcpStream,
        ftt: &mut FileTransferType,
        waker: &Waker,
        cmd_connection_token: Token,
    ) -> Result<(), Error> {
        match ftt {
            FileTransferType::Buffer(to_write) => {
                self.write_buffer_file_transfer(stream, to_write, waker, cmd_connection_token)
            }

            // TODO Handle chunks!!!!!!
            FileTransferType::FileDownload(file) => {
                let mut buf = [0; 1024];
                loop {
                    let read = file.read(&mut buf);
                    if read.is_err() {
                        //...
                        panic!("Unhandled error");
                    }
                    let read = read.unwrap();
                    if read == 0 {
                        break;
                    }
                    let err = stream.write(&buf[0..read]);
                    if let Err(err) = &err {
                        if err.kind() == ErrorKind::WouldBlock {
                            let err_seek = file.seek(SeekFrom::Current(-(read as i64)));
                            if err_seek.is_err() {
                                println!("[ERROR SEEK] Unknown error with seek :( {:?}", err_seek);
                                let _ = self.close_connection(stream);
                                self.answer_command(
                                    cmd_connection_token,
                                    "Unknown error with file transfer",
                                );
                                return Ok(());
                            }
                            println!(
                                "[HANDLE_FILE_TRANSFER] {} - Is would block, let's write again",
                                self.connection_token.0
                            );
                            self.actions.push((
                                self.connection_token,
                                self.connection.clone(),
                                Interest::WRITABLE,
                            ));
                            return Ok(());
                        } else {
                            println!("[HANDLE_FILE_TRANSFER] Error transfering file {:?}", err);
                            let _ = self.close_connection(stream);
                            self.answer_command(
                                cmd_connection_token,
                                "Error with file transfer connection",
                            );
                        }
                    } else {
                        let read_end = err.unwrap();
                        assert!(read_end == read);
                    }
                }
                let _ = self.close_connection(stream);
                self.answer_command(
                    cmd_connection_token,
                    "Closing data connection. Requested file action successful. (file transfer)",
                );
                Ok(())
            }

            _ => unimplemented!(),
        }
    }

    fn write_buffer_file_transfer(
        &mut self,
        stream: &mut TcpStream,
        to_write: &mut BufferToWrite,
        waker: &Waker,
        cmd_connection_token: Token,
    ) -> Result<(), Error> {
        let written = stream.write(&to_write.buffer[to_write.offset..]);
        if let Ok(written) = written {
            println!(
                "[WRITE_BUFFER_FILE_TRANSFER] {} - {} bytes written",
                self.connection_token.0, written
            );
            if written + to_write.offset >= to_write.buffer.len() {
                stream.shutdown(Shutdown::Both)?;
                let mut map_conn = self.connection_db.lock().unwrap();
                assert!(map_conn.remove(&self.connection_token).is_some());
                let command_connection = map_conn.get(&cmd_connection_token);
                if let Some(command_connection) = command_connection {
                    let mut command_connection_mutex = command_connection.lock().unwrap();
                    if let RequestType::CommandTransfer(_, buffer_to_write, t) =
                        &mut command_connection_mutex.request_type
                    {
                        t.take();
                        println!(
                            "[WRITE_BUFFER_FILE_TRANSFER] {} - Succesfully sending to the client, sending close data connection for token: {:?}", 
                            self.connection_token.0,
                            cmd_connection_token
                        );
                        buffer_to_write.buffer = create_response(
                            Response::closing_data_connection(),
                            "Closing data connection. Requested file action successful (for example, file transfer or file abort).",
                        );
                        buffer_to_write.offset = 0;
                    } else {
                        println!("[WRITE_BUFFER_FILE_TRANSFER] {} - Unexpected request type for command transfer", self.connection_token.0);
                    }
                    drop(command_connection_mutex);
                    self.actions.push((
                        cmd_connection_token,
                        command_connection.clone(),
                        Interest::WRITABLE,
                    ));
                } else {
                    println!(
                        "[WRITE_BUFFER_FILE_TRANSFER] {} - Not found connection in DB",
                        self.connection_token.0
                    );
                }
                return Ok(());
            }
            to_write.offset += written;
            self.keep_interest(waker, Interest::WRITABLE)?;
            println!(
                "[WRITE_BUFFER_FILE_TRANSFER] {} - Keep writing...",
                self.connection_token.0
            );
        } else if let Err(err) = written {
            if err.kind() == ErrorKind::WouldBlock {
                std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("./debug_write.txt")
                    .unwrap()
                    .write(format!("\nWRITE {:?}", err).as_bytes())
                    .unwrap();
                println!(
                    "[WRITE_BUFFER_FILE_TRANSFER] {} - Would block error, keep writing",
                    self.connection_token.0
                );
                self.keep_interest(waker, Interest::WRITABLE)?;
            } else {
                println!(
                    "[WRITE_BUFFER_FILE_TRANSFER] {} - Closing connection because {}",
                    self.connection_token.0, err
                );
                self.close_connection(stream)?;
            }
        }
        Ok(())
    }
}
