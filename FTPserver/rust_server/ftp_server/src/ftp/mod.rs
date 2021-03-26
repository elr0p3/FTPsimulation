use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::MutexGuard,
};

use mio::{event::Event, Interest, Poll, Token};
use mio::{
    event::Source,
    net::{TcpListener, TcpStream},
};
use std::io::{Error, ErrorKind};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};

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

#[derive(Default, Debug, Clone)]
struct BufferToWrite {
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

/// We need to think about still
/// - concurrency (we need to adapt things to concurrency)
/// - storing user state (what do we need?)
/// - storing file state in file transfer
/// - Find a way to increment the tokenId
// TODO: Create user struct and all of that logic so we can keep a reference to a user in the request_context
#[derive(Debug)]
enum RequestType {
    /// This requesst is a file transfer on passive mode.
    /// The token on the right is the identifier for the server listener!
    FileTransferPassive(TcpStream, BufferToWrite),

    FileTransferActive(TcpStream, BufferToWrite),

    CommandTransfer(TcpStream, BufferToWrite),

    /// This is the passive mode port that will accept connections
    PassiveModePort(TcpListener),
}

struct RequestContext {
    request_type: RequestType,
    // socket_addr: SocketAddr,
}

impl RequestContext {
    fn new(request_type: RequestType) -> Self {
        Self { request_type }
    }
}

type RequestContextMutex = Arc<Mutex<RequestContext>>;

type HashMutex<K, V> = Arc<Mutex<HashMap<K, V>>>;

pub struct FTPServer {
    /// We will need to put this an ArcMutex
    connections: HashMutex<Token, RequestContextMutex>,
    current_id: usize,
    port: usize,
}

impl FTPServer {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            current_id: 0,
            port: 50_000,
        }
    }

    fn add_connection(&mut self, token: Token, request_type: RequestType) {
        self.connections.lock().unwrap().insert(
            token,
            Arc::new(Mutex::new(RequestContext::new(request_type))),
        );
    }

    fn new_passive_listener(&mut self, poll: &Poll) -> Result<(), String> {
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
        self.add_connection(Token(id), RequestType::PassiveModePort(listener));
        Ok(())
    }
}

impl TCPImplementation for FTPServer {
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
            .register(&mut stream, token, Interest::READABLE)?;
        self.add_connection(
            token,
            RequestType::CommandTransfer(stream, BufferToWrite::default()),
        );
        Ok(())
    }

    fn write_connection(&mut self, poll: &Poll, event: &Event) -> Result<(), Error> {
        let map_conn = self.connections.clone();
        let map_conn = map_conn.lock().unwrap();
        let conn = {
            let connection = map_conn.get(&event.token()).ok_or(ErrorKind::NotFound)?;
            let arc = connection.clone();
            arc
        };
        let mut conn = conn.lock().unwrap();
        // let buff = get_test_html("hello world!");
        match &mut conn.request_type {
            RequestType::CommandTransfer(stream, to_write) => {
                let written = stream.write(&to_write.buffer[to_write.offset..])?;
                println!("writing! {}", written);
                // Close connection, everything is written
                if written + to_write.offset >= to_write.buffer.len() {
                    stream.reregister(poll.registry(), event.token(), Interest::READABLE)?;
                }
                // We would need to handle some offset, but atm with testing HTML we just do this
                Ok(())
            }

            // NOTE: This will have custom behaviours in the future
            RequestType::FileTransferPassive(stream, to_write) => {
                let written = stream.write(&to_write.buffer[to_write.offset..])?;
                println!("writing file transfer! {}", written);
                if written + to_write.offset >= to_write.buffer.len() {
                    stream.shutdown(Shutdown::Both)?;
                    stream.deregister(poll.registry())?;
                    return Ok(());
                }
                to_write.offset += written;
                Ok(())
            }
            _ => Err(Error::from(ErrorKind::NotFound)),
        }
    }

    fn read_connection(&mut self, poll: &Poll, event: &Event) -> Result<(), Error> {
        // first read
        let map_conn = self.connections.clone();
        let map_conn = map_conn.lock().unwrap();
        let conn = {
            let connection = map_conn.get(&event.token()).ok_or(ErrorKind::NotFound)?;
            let arc = connection.clone();
            arc
        };
        // drop mutex
        drop(map_conn);
        let mut conn = conn.lock().unwrap();
        match &mut conn.request_type {
            RequestType::CommandTransfer(stream, to_write) => {
                let mut buff = [0; 10024];
                let read = stream.read(&mut buff)?;
                println!("Read buffer: {}", read);
                // temporal condition
                if read >= buff.len() {
                    // Just close connection if the request is too big at the moment
                    return Err(Error::from(ErrorKind::Other));
                }
                if read == 5 {
                    self.new_passive_listener(poll)
                        .map_err(|_| ErrorKind::InvalidData)?;
                    println!("** New port on {}", self.port - 1);
                    to_write.buffer.append(&mut get_test_html(
                        format!("Connect to port: {}", self.port - 1).as_str(),
                    ));
                } else {
                    to_write.buffer.append(&mut get_test_html("HI"));
                }
                poll.registry().deregister(stream)?;
                poll.registry()
                    .register(stream, event.token(), Interest::WRITABLE)?;
                Ok(())
            }

            RequestType::PassiveModePort(listener) => {
                let (mut stream, _addr) = listener.accept()?;
                let tok = Token(self.next_id());
                poll.registry()
                    .register(&mut stream, tok, Interest::WRITABLE)?;
                self.add_connection(
                    tok,
                    RequestType::FileTransferPassive(
                        stream,
                        BufferToWrite::new(get_test_html("HELLO")),
                    ),
                );
                // Remove the listener
                self.connections.lock().unwrap().remove(&event.token());
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
        let mut conn = conn.lock().unwrap();
        match &mut conn.request_type {
            RequestType::FileTransferActive(stream, _)
            | RequestType::FileTransferPassive(stream, _)
            | RequestType::CommandTransfer(stream, _) => {
                poll.registry().deregister(stream)?;
                stream.shutdown(Shutdown::Both)?;
                println!("connection with the client was closed");
            }
            RequestType::PassiveModePort(stream) => {
                // We actually just deregister when we write
                poll.registry().deregister(stream)?;
                println!("closed a connection!");
            }
        }
        self.connections.lock().unwrap().remove(&token);
        Ok(())
    }
}
