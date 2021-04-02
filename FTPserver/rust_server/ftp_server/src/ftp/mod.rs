use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

mod command;
mod handler_read;
mod handlers;
mod response;
use command::Command;
use response::Response;

// use handlers::write_buffer_file_transfer;
use mio::net::{TcpListener, TcpStream};
use mio::{event::Event, Interest, Poll, Token, Waker};
use std::convert::TryFrom;
use std::io::{Error, ErrorKind};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use crate::tcp::TCPImplementation;

use self::{
    handler_read::HandlerRead,
    handlers::{close_connection_recursive, HandlerWrite},
};

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

/// Buffer that is really useful to set to a writable request_context
pub struct BufferToWrite {
    /// Total data that this buffer is gonna send
    buffer: Vec<u8>,

    /// Current offset of the buffer
    offset: usize,

    /// We are using this callback mainly to do an action just after sending a command
    /// For example if we send a transition command 1XX, and make sure that just after that
    /// we start a file transfer, we need to pass a threadsafe callback that will start that action
    /// (For example starting a writable interest to the file transfer socket)
    /// Make sure that you use `.take()` for emptying the option
    callback_after_sending: Option<Box<dyn FnOnce() + Send>>,
}

impl BufferToWrite {
    fn default() -> Self {
        Self {
            buffer: Vec::default(),
            offset: 0,
            callback_after_sending: None,
        }
    }

    fn new(vector: Vec<u8>) -> Self {
        Self {
            buffer: vector,
            offset: 0,
            callback_after_sending: None,
        }
    }

    fn reset(&mut self, vector: Vec<u8>) {
        self.buffer = vector;
        self.offset = 0;
    }

    fn reset_str(&mut self, vector: &str) {
        self.buffer = vector.as_bytes().to_vec();
        self.offset = 0;
    }
}

// #[derive(Debug)]
pub enum FileTransferType {
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
// #[derive(Debug)]
pub enum RequestType {
    /// This requesst is a file transfer on passive mode.

    /// Also the token is for referencing the `CommandTransfer` req_ctx connection
    /// so we can send a command when the download is finished!
    FileTransferPassive(TcpStream, FileTransferType, Token),

    /// This requesst is a file transfer on active mode.    
    /// Also the token is for referencing the `CommandTransfer` req_ctx connection
    /// so we can send a command when the download is finished!
    FileTransferActive(TcpStream, FileTransferType, Token),

    /// TcpStream of the connection
    /// BufferToWrite is the buffer that is gonna be written on Write mode
    /// Option<Token> is the opened PassiveModePort/FileTransferActive/FileTransferPassive
    CommandTransfer(TcpStream, BufferToWrite, Option<Token>),

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

pub const ROOT: &'static str = "./root";

impl FTPServer {
    pub fn new() -> Self {
        if !Path::new(ROOT).exists() {
            fs::create_dir(ROOT).expect("root dir hasn't been created");
        }
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
            RequestType::CommandTransfer(stream, _, _) => {
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

    fn deregister_and_shutdown(&self, poll: &Poll, rc: &mut RequestContext) -> Result<(), Error> {
        let _ = self.deregister(poll, rc);
        match &mut rc.request_type {
            RequestType::CommandTransfer(stream, _, _) => {
                stream.shutdown(Shutdown::Both)?;
            }

            RequestType::FileTransferActive(stream, _, _) => {
                stream.shutdown(Shutdown::Both)?;
            }

            RequestType::FileTransferPassive(stream, _, _) => {
                stream.shutdown(Shutdown::Both)?;
            }

            RequestType::PassiveModePort(port, _) => {}
        }
        Ok(())
    }

    fn action_add(actions: &ActionList, action: Action) {
        println!("adding an action...");
        let mut actions_locked = actions.lock().unwrap();
        actions_locked.push(action);
        println!("added action");
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
                None,
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
        let actions_ref = self.action_list();
        spawn(move || {
            let mut conn = connection.lock().unwrap();
            let mut handler = HandlerWrite::new(token, map_conn_arc.clone(), connection.clone());
            if let Err(err) = handler.handle_write(&mut conn.request_type, &waker) {
                println!("fatal error {}", err);
            }
            drop(conn);
            println!("Adding actions!");
            let mut actions_locked = actions_ref.lock().unwrap();
            for action in handler.actions {
                actions_locked.push(action);
                let _ = waker.wake();
            }
            println!("Finished adding actions!");
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
        let mut handler_read = HandlerRead::new(token, self.connections.clone(), conn.clone());
        let actions = self.action_list();
        let next_id = self.next_id();
        spawn(move || {
            let connection_arc = conn.clone();
            let mut connection_mutex = connection_arc.lock().unwrap();
            let response = handler_read.handle_read(
                &mut connection_mutex.request_type,
                &waker,
                actions.clone(),
                next_id,
            );
            // If it's a definitive error
            let is_err = response.is_err() && response.unwrap_err().kind() != ErrorKind::WouldBlock;
            if is_err {
                println!("Closing read because of an error...");
                let _ = close_connection_recursive(
                    handler_read.connection_db.clone(),
                    handler_read.connection_token,
                );
                let _ = waker.wake();
            } else {
                drop(connection_mutex);
                let mut actions = actions.lock().unwrap();
                for action in handler_read.actions {
                    println!("pushing action...");
                    actions.push(action);
                    let _ = waker.wake();
                }
            }
            println!("Finished doing read of request_context");
        });
        Ok(())
    }

    fn close_connection(&mut self, poll: &Poll, token: Token) -> Result<(), Error> {
        let map_conn_arc = self.connections.clone();
        let map_conn = map_conn_arc.lock().unwrap();
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
            | RequestType::FileTransferPassive(stream, _, _) => {
                poll.registry().deregister(stream)?;
                stream.shutdown(Shutdown::Both)?;
                println!("connection with the client was closed");
            }
            RequestType::CommandTransfer(stream, _, conn) => {
                println!("Connection with the client (COMMAND) is closed");
                // Ignore error to be honest, don't care if we try to close twice
                let _ = poll.registry().deregister(stream);
                let _ = stream.shutdown(Shutdown::Both);
                let conn = conn.take();
                if let Some(conn) = &conn {
                    let mut map_conn = map_conn_arc.lock().unwrap();
                    let connection = map_conn.get_mut(conn);
                    if let Some(connection) = connection {
                        println!("Disconnecting from dangling transfer connection");
                        let mut connection = connection.lock().unwrap();
                        // Don't care if we close twice
                        let _ = self.deregister_and_shutdown(poll, &mut connection);
                        drop(connection);
                        map_conn.remove(conn);
                    }
                }
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

#[cfg(test)]
mod ftp_server_testing {
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::net::TcpStream;
    // use mio::net::{SocketAddr, TcpListener};

    fn expect_response(stream: &mut TcpStream, response_expects: &str) {
        let mut buff = [0; 1024];
        let read = stream.read(&mut buff).expect("read didn't go well");
        let str = std::str::from_utf8(&buff[0..read]).expect("error parsing response");
        assert_eq!(response_expects, str);
    }

    #[test]
    fn it_works() {
        for i in 0..100 {
            let result = TcpStream::connect("127.0.0.1:8080");
            if let Err(err) = result {
                panic!("{}", err);
            }
            let mut stream = result.unwrap();
            expect_response(&mut stream, "220 Service ready for new user.\r\n");
            let srv = TcpListener::bind("127.0.0.1:2235").expect("to create server");
            stream
                .write_all(&"PORT 127,0,0,1,8,187\r\n".as_bytes())
                .expect("writing everything");
            let join = std::thread::spawn(move || {
                let (mut conn, _) = srv.accept().expect("expect to receive connection");
                let mut buff = [0; 1024];
                let read = conn.read(&mut buff).expect("to have read");
                assert_eq!(read, 1000);
                assert_eq!(buff[0], 1);
                let possible_err = conn.read(&mut buff);
                assert!(possible_err.unwrap() == 0);
            });
            expect_response(&mut stream, "200 Command okay.\r\n");
            stream
                .write_all(&"LIST\r\n".as_bytes())
                .expect("writing everything");
            expect_response(
                &mut stream,
                "150 File status okay; about to open data connection.\r\n",
            );
            expect_response(&mut stream, "226 Closing data connection. Requested file action successful (for example, file transfer or file abort).\r\n");
            join.join().unwrap();
        }
    }
}
