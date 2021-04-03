use super::{
    create_response, Action, ActionList, BufferToWrite, FTPServer, HashMutex, RequestContextMutex,
    RequestType, Token,
};
use super::{response::Response, FileTransferType};
use mio::{net::TcpStream, Interest, Waker};
use std::io::{ErrorKind, Write};
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
    let mut conn = conn.lock().unwrap();
    connection_database.lock().unwrap().remove(&to_delete);
    println!("[CLOSED CONNECTION]");
    match &mut conn.request_type {
        RequestType::FileTransferActive(stream, _, _)
        | RequestType::FileTransferPassive(stream, _, _) => {
            let _ = stream.shutdown(Shutdown::Both)?;
            println!("connection with the client was closed");
        }
        RequestType::CommandTransfer(stream, _, conn) => {
            println!("connection with the client was closed");
            // Ignore error to be honest, don't care if we try to close twice
            let _ = stream.shutdown(Shutdown::Both);
            let conn = conn.take();
            if let Some(conn) = &conn {
                println!("Closed dangling connections");
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

    pub fn handle_write(
        &mut self,
        request_type: &mut RequestType,
        waker: &Waker,
    ) -> Result<(), Error> {
        match request_type {
            RequestType::CommandTransfer(stream, to_write, t) => {
                let written = stream.write(&to_write.buffer[to_write.offset..]);
                if let Ok(written) = written {
                    println!("writing! {}", written);
                    if written + to_write.offset >= to_write.buffer.len() {
                        to_write.buffer.clear();
                        to_write.offset = 0;
                        self.keep_interest(waker, Interest::READABLE)?;
                        if let Some(callback) = to_write.callback_after_sending.take() {
                            callback();
                        }
                    } else {
                        // Keep writing
                        to_write.offset += written;
                        self.keep_interest(waker, Interest::WRITABLE)?;
                    }
                } else if let Err(err) = written {
                    if err.kind() == ErrorKind::WouldBlock {
                        self.keep_interest(waker, Interest::WRITABLE)?;
                    } else {
                        self.close_connection(stream)?;
                        if let Some(t) = t {
                            close_connection_recursive(self.connection_db.clone(), *t)?;
                        }
                        println!("error writing because: {}", err);
                    }
                }
            }

            RequestType::FileTransferPassive(stream, ftt, conn_tok) => {
                self.handle_file_transfer(stream, ftt, waker, *conn_tok)?;
            }

            RequestType::FileTransferActive(stream, ftt, conn_tok) => {
                self.handle_file_transfer(stream, ftt, waker, *conn_tok)?;
            }

            _ => return Err(Error::from(ErrorKind::NotFound)),
        }
        Ok(())
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
                // TODO Handle errors
                let _ = std::io::copy(file, stream);
                let _ = self.close_connection(stream);
                let mut db = self.connection_db.lock().unwrap();
                let cmd = db.get_mut(&cmd_connection_token);
                if let Some(cmd) = cmd {
                    let cmd_arc = cmd.clone();
                    let mut cmd = cmd_arc.lock().unwrap();
                    if let RequestType::CommandTransfer(_stream, to_write, t) =
                        &mut cmd.request_type
                    {
                        t.take();
                        to_write.reset(create_response(
                            Response::closing_data_connection(),
                            "Closing data connection. Requested file action successful. (file transfer)",
                        ));
                        self.actions.push((
                            cmd_connection_token,
                            cmd_arc.clone(),
                            Interest::WRITABLE,
                        ));
                    }
                }
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
            println!("Writing file transfer! {}", written);
            if written + to_write.offset >= to_write.buffer.len() {
                println!("Closing write connection...");
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
                        println!("Succesfully sending to the client!");
                        buffer_to_write.buffer = create_response(
                            Response::closing_data_connection(),
                            "Closing data connection. Requested file action successful (for example, file transfer or file abort).",
                        );
                        buffer_to_write.offset = 0;
                    } else {
                        println!("Unexpected request type for command transfer");
                    }
                    drop(command_connection_mutex);
                    self.actions.push((
                        cmd_connection_token,
                        command_connection.clone(),
                        Interest::WRITABLE,
                    ));
                } else {
                    println!("not found connection...");
                }
                return Ok(());
            }
            to_write.offset += written;
            self.keep_interest(waker, Interest::WRITABLE)?;
        } else if let Err(err) = written {
            if err.kind() == ErrorKind::WouldBlock {
                self.keep_interest(waker, Interest::WRITABLE)?;
            } else {
                self.close_connection(stream)?;
            }
        }
        Ok(())
    }
}