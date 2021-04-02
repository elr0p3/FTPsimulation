use super::{
    create_response, ActionList, BufferToWrite, FTPServer, HashMutex, RequestContextMutex,
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
    match &mut conn.request_type {
        RequestType::FileTransferActive(stream, _, _)
        | RequestType::FileTransferPassive(stream, _, _) => {
            stream.shutdown(Shutdown::Both)?;
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

    actions_reference: ActionList,

    connection: RequestContextMutex,
}

impl HandlerWrite {
    pub fn new(
        connection_token: Token,
        connection_db: HashMutex<Token, RequestContextMutex>,
        actions_reference: ActionList,
        connection: RequestContextMutex,
    ) -> Self {
        Self {
            connection_token,
            connection_db,
            actions_reference,
            connection,
        }
    }

    fn keep_interest(&mut self, waker: &Waker, interest: Interest) -> Result<(), Error> {
        FTPServer::action_add(
            &self.actions_reference,
            (self.connection_token, self.connection.clone(), interest),
        );
        waker.wake()?;
        Ok(())
    }

    fn close_connection(&mut self, stream: &mut TcpStream) -> Result<(), Error> {
        stream.shutdown(Shutdown::Both)?;
        let mut map_conn = self.connection_db.lock().unwrap();
        map_conn.remove(&self.connection_token);
        Ok(())
    }

    pub fn handle_write(
        mut self,
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
                        self.keep_interest(waker, Interest::READABLE)?;
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
                            close_connection_recursive(self.connection_db, *t)?;
                        }
                        println!("error writing because: {}", err);
                    }
                }
                Ok(())
            }

            RequestType::FileTransferPassive(stream, ftt, conn_tok) => {
                self.handle_file_transfer(stream, ftt, waker, *conn_tok)
            }
            _ => Err(Error::from(ErrorKind::NotFound)),
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
            println!("writing file transfer! {}", written);
            if written + to_write.offset >= to_write.buffer.len() {
                stream.shutdown(Shutdown::Both)?;
                let map_conn = self.connection_db.lock().unwrap();
                let command_connection = map_conn.get(&cmd_connection_token);
                if let Some(command_connection) = command_connection {
                    let mut command_connection_mutex = command_connection.lock().unwrap();
                    if let RequestType::CommandTransfer(_, buffer_to_write, _) =
                        &mut command_connection_mutex.request_type
                    {
                        println!("succesfully sending to the client!");
                        buffer_to_write.buffer = create_response(
                            Response::success_transfering_file(),
                            "Successfully transfered file...",
                        );
                        buffer_to_write.offset = 0;
                        FTPServer::action_add(
                            &self.actions_reference,
                            (
                                cmd_connection_token,
                                command_connection.clone(),
                                Interest::WRITABLE,
                            ),
                        );
                        waker.wake()?;
                    } else {
                        println!("unexpected request type for command transfer");
                    }
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
