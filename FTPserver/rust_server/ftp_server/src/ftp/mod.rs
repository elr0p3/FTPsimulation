use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
};

mod command;
mod response;

use command::Command;
use response::Response;

use mio::net::{TcpListener, TcpStream};
use mio::{event::Event, Interest, Poll, Token, Waker};
use std::io::{Error, ErrorKind};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use crate::tcp::TCPImplementation;

fn get_test_html(data: &str) -> Vec<u8> {
    return format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        data.len(),
        data
    )
    .as_bytes()
    .to_vec();
}

fn create_response(response_code: Response, message: &str) -> Vec<u8> {
    format!("{} {}\r\n", response_code.0, message).into_bytes()
}

#[derive(Default, Debug, Clone)]
pub struct BufferToWrite {
    buffer: Vec<u8>,
    offset: usize,
}

impl BufferToWrite {
    fn new(vector: Vec<u8>) -> Self {
        Self {
            buffer: vector,
            offset: 0,
        }
    }
}

enum FileTransferType {
    /// This kind of operation is when the server is saving a file from the client
    FileUpload(File),

    /// This kind of operation is when the server is serving a file to the client
    FileDownload(File),

    /// This kind of operation is when the server is just writing some data to the client
    Buffer(BufferToWrite),
}

/// We need to think about still
/// - storing user state (what do we need?)
/// - storing file state in file transfer
// TODO: Create user struct and all of that logic so we can keep a reference to a user in the request_context
#[derive(Debug)]
pub enum RequestType {
    /// This requesst is a file transfer on passive mode.

    /// Also the token is for referencing the `CommandTransfer` req_ctx connection
    /// so we can send a command when the download is finished!
    FileTransferPassive(TcpStream, BufferToWrite, Token),

    /// This requesst is a file transfer on active mode.
    /// The token on the right is the identifier for the server listener!
    /// Also the token is for referencing the `CommandTransfer` req_ctx connection
    /// so we can send a command when the download is finished!
    FileTransferActive(TcpStream, BufferToWrite, Token),

    CommandTransfer(TcpStream, BufferToWrite),

    /// This is the passive mode port that will accept connections
    /// It has a token where it references the CommandTransfer request_ctx
    PassiveModePort(TcpListener, Token),
}

pub struct RequestContext {
    pub request_type: RequestType,
    // (note): would be cool to have here the user_id reference when creating the user
    // socket_addr: SocketAddr,
}

impl RequestContext {
    fn new(request_type: RequestType) -> Self {
        Self { request_type }
    }
}

pub type RequestContextMutex = Arc<Mutex<RequestContext>>;

type Action = (Token, RequestContextMutex, Interest);

type ActionList = Arc<Mutex<Vec<Action>>>;

type HashMutex<K, V> = Arc<Mutex<HashMap<K, V>>>;

pub struct FTPServer {
    connections: HashMutex<Token, RequestContextMutex>,
    actions: ActionList,
    current_id: usize,
    port: usize,
}

impl FTPServer {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            current_id: 0,
            port: 50_000,
            actions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_connection(&mut self, token: Token, request_type: RequestType) {
        self.connections.lock().unwrap().insert(
            token,
            Arc::new(Mutex::new(RequestContext::new(request_type))),
        );
    }

    fn new_passive_listener(
        &mut self,
        poll: &Poll,
        command_transfer_conn: Token,
    ) -> Result<(), String> {
        let port = self.port;
        self.port += 1;
        let id = self.next_id();
        let mut listener = TcpListener::bind(
            format!("127.0.0.1:{}", port)
                .parse()
                .map_err(|_| format!("can't bind to this address"))?,
        )
        .map_err(|_| format!("can't bind to this port"))?;
        poll.registry()
            .register(&mut listener, Token(id), Interest::READABLE)
            .map_err(|_| format!("cannot register this socket"))?;
        self.add_connection(
            Token(id),
            RequestType::PassiveModePort(listener, command_transfer_conn),
        );
        Ok(())
    }

    fn deregister(&self, poll: &Poll, rc: &mut RequestContext) -> Result<(), Error> {
        match &mut rc.request_type {
            RequestType::CommandTransfer(stream, _) => {
                poll.registry().deregister(stream)?;
            }

            RequestType::FileTransferActive(stream, _, _) => {
                poll.registry().deregister(stream)?;
            }

            RequestType::FileTransferPassive(stream, _, _) => {
                poll.registry().deregister(stream)?;
            }

            RequestType::PassiveModePort(port, _) => {
                poll.registry().deregister(port)?;
            }
        }
        Ok(())
    }

    fn action_add(actions: &ActionList, action: Action) {
        let mut actions_locked = actions.lock().unwrap();
        actions_locked.push(action);
    }

    fn handle_file_transfer_passive(
        action_list: ActionList,
        waker: Arc<Waker>,
        request_context: RequestContextMutex,
    ) -> Result<(), Error> {
        let mut request_ctx_mutex = request_context.lock().map_err(|_| ErrorKind::Other)?;
        if let RequestType::FileTransferPassive(stream, internal, _token_request_commands) =
            &mut request_ctx_mutex.request_type
        {}
        unimplemented!()
    }
}

impl TCPImplementation for FTPServer {
    fn action_list(&mut self) -> Arc<Mutex<Vec<Action>>> {
        self.actions.clone()
    }

    fn next_id(&mut self) -> usize {
        self.current_id += 1;
        self.current_id
    }

    fn new_connection(
        &mut self,
        _: Token,
        token: Token,
        poll: &Poll,
        mut stream: TcpStream,
    ) -> Result<(), std::io::Error> {
        println!("new connection!");
        poll.registry()
            .register(&mut stream, token, Interest::WRITABLE)?;
        self.add_connection(
            token,
            RequestType::CommandTransfer(
                stream,
                BufferToWrite::new(create_response(
                    Response::service_ready(),
                    "Service ready for new user.",
                )),
            ),
        );
        Ok(())
    }

    fn write_connection(
        &mut self,
        poll: &Poll,
        waker: Arc<Waker>,
        event: &Event,
    ) -> Result<(), Error> {
        // TODO Make this a macro!
        let map_conn_arc = self.connections.clone();
        let token = event.token();
        let map_conn = map_conn_arc.lock().unwrap();
        let connection = {
            let connection = map_conn.get(&token).ok_or(ErrorKind::NotFound)?;
            let arc = connection.clone();
            arc
        };
        drop(map_conn);
        let mut connection_mutex = connection.lock().unwrap();
        self.deregister(poll, &mut connection_mutex)?;
        drop(connection_mutex);
        let actions_ref = self.action_list().clone();
        spawn(move || {
            let mut conn = connection.lock().unwrap();
            match &mut conn.request_type {
                RequestType::CommandTransfer(stream, to_write) => {
                    let written = stream.write(&to_write.buffer[to_write.offset..]);
                    if let Ok(written) = written {
                        println!("writing! {}", written);
                        if written + to_write.offset >= to_write.buffer.len() {
                            to_write.buffer.clear();
                            FTPServer::action_add(
                                &actions_ref,
                                (token, connection.clone(), Interest::READABLE),
                            );
                            waker.wake()?;
                        } else {
                            // Keep writing
                            to_write.offset += written;
                            FTPServer::action_add(
                                &actions_ref,
                                (token, connection.clone(), Interest::WRITABLE),
                            );
                            waker.wake()?;
                        }
                    } else if let Err(err) = written {
                        if err.kind() == ErrorKind::WouldBlock {
                            FTPServer::action_add(
                                &actions_ref,
                                (token, connection.clone(), Interest::WRITABLE),
                            )
                        } else {
                            stream.shutdown(Shutdown::Both)?;
                            println!("error writing because: {}", err);
                        }
                    }
                    Ok(())
                }

                // NOTE: This will have custom behaviours in the future
                // This is a demo of how it should behave, we should have lots of custom behaviours to be honest :|
                RequestType::FileTransferPassive(stream, to_write, conn_tok) => {
                    let written = stream.write(&to_write.buffer[to_write.offset..]);
                    if let Ok(written) = written {
                        println!("writing file transfer! {}", written);
                        if written + to_write.offset >= to_write.buffer.len() {
                            stream.shutdown(Shutdown::Both)?;
                            let map_conn = map_conn_arc.lock().unwrap();
                            let command_connection = map_conn.get(&conn_tok);
                            if let Some(command_connection) = command_connection {
                                let command_connection = command_connection.clone();
                                let other_command_connection = command_connection.clone();
                                let mut command_connection_mutex =
                                    command_connection.lock().unwrap();
                                if let RequestType::CommandTransfer(_, buffer_to_write) =
                                    &mut command_connection_mutex.request_type
                                {
                                    println!("succesfully sending to the client!");
                                    buffer_to_write.buffer = create_response(
                                        Response::success_transfering_file(),
                                        "Successfully transfered file...",
                                    );
                                    buffer_to_write.offset = 0;
                                    FTPServer::action_add(
                                        &actions_ref,
                                        (*conn_tok, other_command_connection, Interest::WRITABLE),
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
                        FTPServer::action_add(
                            &actions_ref,
                            (token, connection.clone(), Interest::WRITABLE),
                        );
                        waker.wake()?;
                    } else if let Err(err) = written {
                        if err.kind() == ErrorKind::WouldBlock {
                            FTPServer::action_add(
                                &actions_ref,
                                (token, connection.clone(), Interest::WRITABLE),
                            );
                        } else {
                            stream.shutdown(Shutdown::Both)?;
                            let mut map_conn = map_conn_arc.lock().unwrap();
                            map_conn.remove(&token);
                        }
                    }
                    Ok(())
                }
                _ => Err(Error::from(ErrorKind::NotFound)),
            }
        });
        Ok(())
    }

    fn read_connection(
        &mut self,
        poll: &Poll,
        waker: Arc<Waker>,
        event: &Event,
    ) -> Result<(), Error> {
        // first read
        let map_conn = self.connections.clone();
        let map_conn = map_conn.lock().unwrap();
        let conn = {
            let connection = map_conn.get(&event.token()).ok_or(ErrorKind::NotFound)?;
            let arc = connection.clone();
            arc
        };
        let token = event.token();
        drop(map_conn);
        let mut conn = conn.lock().unwrap();
        match &mut conn.request_type {
            RequestType::CommandTransfer(stream, to_write) => {
                // Initialize a big buffer
                let mut buff = [0; 10024];

                // Read thing into the buffer TODO Handle block in multithread
                let read = stream.read(&mut buff)?;

                println!("Read buffer: {}", read);

                // Testing condition
                if read >= buff.len() {
                    // Just close connection if the request is too big at the moment
                    return Err(Error::from(ErrorKind::Other));
                }

                // Another testing condition where we just check that passive listeners work
                // we have to create a function `handle_client_ftp_command`
                if read == 5 {
                    // In the future we also might have to put here the kind of passive listener we want
                    self.new_passive_listener(poll, token)
                        .map_err(|_| ErrorKind::InvalidData)?;

                    println!("** New port on {}", self.port - 1);

                    // Test data
                    to_write.buffer.append(&mut get_test_html(
                        format!("Connect to port: {}", self.port - 1).as_str(),
                    ));

                    return Ok(());
                } else {
                    to_write.buffer.append(&mut get_test_html("HI"));
                }
                poll.registry().deregister(stream)?;
                poll.registry()
                    .register(stream, event.token(), Interest::WRITABLE)?;
                Ok(())
            }

            RequestType::PassiveModePort(listener, command_conn_ref) => {
                // Accept file connection
                let (mut stream, _addr) = listener.accept()?;

                // Get the token for the connection
                let token_for_connection = Token(self.next_id());

                // Register the connection as writable/readable
                // TODO Note that we need to put in passivemodeport the field of which kind of connection is this
                // (Download, Upload, Just Buffer Transfer...)
                poll.registry()
                    .register(&mut stream, token_for_connection, Interest::WRITABLE)?;

                // Add the connection
                self.add_connection(
                    token_for_connection,
                    RequestType::FileTransferPassive(
                        stream,
                        BufferToWrite::new(get_test_html("HELLO")),
                        *command_conn_ref,
                    ),
                );

                // Remove the listener (won't accept more connections)
                self.connections.lock().unwrap().remove(&event.token());

                // Just deregister
                poll.registry().deregister(listener)?;

                Ok(())
            }

            _ => unimplemented!("Unimplemented Request type: {:?}", conn.request_type),
        }
    }

    fn close_connection(&mut self, poll: &Poll, token: Token) -> Result<(), Error> {
        let map_conn = self.connections.clone();
        let map_conn = map_conn.lock().unwrap();
        let conn = {
            let connection = map_conn.get(&token);
            if connection.is_none() {
                return Ok(());
            }
            let arc = connection.unwrap().clone();
            arc
        };
        drop(map_conn);
        let mut conn = conn.lock().unwrap();
        self.connections.lock().unwrap().remove(&token);
        match &mut conn.request_type {
            RequestType::FileTransferActive(stream, _, _)
            | RequestType::FileTransferPassive(stream, _, _)
            | RequestType::CommandTransfer(stream, _) => {
                poll.registry().deregister(stream)?;
                stream.shutdown(Shutdown::Both)?;
                println!("connection with the client was closed");
            }
            RequestType::PassiveModePort(stream, _) => {
                // We actually just deregister when we write
                poll.registry().deregister(stream)?;
                println!("closed a connection!");
            }
        }
        Ok(())
    }
}
