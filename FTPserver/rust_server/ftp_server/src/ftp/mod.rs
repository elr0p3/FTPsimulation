use std::{
    collections::HashMap,
    io::{Read, Write},
    ops::Deref,
};

use std::io::{Error, ErrorKind};

use std::net::Shutdown;

use mio::net::TcpStream;
use mio::{event::Event, Interest, Poll, Token};

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

/// We need to think about still
/// - concurrency (we need to adapt things to concurrency)
/// - storing user state (what do we need?)
/// - storing file state in file transfer
// TODO: Create user struct and all of that logic so we can keep a reference to a user in the request_context
#[derive(Clone, Copy, Debug)]
enum RequestType {
    FileTransfer,
    CommandTransfer,
    Server,
}

struct RequestContext {
    request_type: RequestType,
    stream: TcpStream,
}

impl RequestContext {
    fn new(stream: TcpStream, request_type: RequestType) -> Self {
        Self {
            stream,
            request_type,
        }
    }
}

pub struct FTPServer {
    connections: HashMap<Token, RequestContext>,
}

impl FTPServer {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    fn add_connection(&mut self, token: Token, tcp_stream: TcpStream, request_type: RequestType) {
        self.connections
            .insert(token, RequestContext::new(tcp_stream, request_type));
    }

    fn get_connection_mut<'a>(&'a mut self, token: Token) -> Option<&'a mut RequestContext> {
        self.connections.get_mut(&token)
    }

    fn get_connection<'a>(&'a self, token: Token) -> Option<&'a RequestContext> {
        self.connections.get(&token)
    }
}

impl TCPImplementation for FTPServer {
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
        self.add_connection(token, stream, RequestType::CommandTransfer);
        Ok(())
    }

    fn write_connection(&mut self, _: &Poll, event: &Event) -> Result<(), Error> {
        if let Some(conn) = self.get_connection_mut(event.token()) {
            let buff = get_test_html("hello world!");
            let written = conn.stream.write(&buff)?;
            println!("writing! {}", written);
            // Close connection, everything is written
            if written >= buff.len() {
                // Just close connection
                return Err(Error::from(ErrorKind::Other));
            }
            // We would need to handle some offset, but atm with testing HTML we just do this
            Ok(())
        } else {
            Err(Error::from(ErrorKind::NotFound))
        }
    }

    fn read_connection(&mut self, poll: &Poll, event: &Event) -> Result<(), Error> {
        if let Some(conn) = self.get_connection_mut(event.token()) {
            let mut buff = [0; 10024];
            let read = conn.stream.read(&mut buff)?;
            println!("read buffer: {}", read);
            // Close connection, everything is written
            if read >= buff.len() {
                println!("f");
                // Just close connection if the request is too big at the moment
                return Err(Error::from(ErrorKind::Other));
            }

            poll.registry().deregister(&mut conn.stream)?;

            // It's good!
            poll.registry()
                .register(&mut conn.stream, event.token(), Interest::WRITABLE)?;

            Ok(())
        } else {
            Err(Error::from(ErrorKind::NotFound))
        }
    }

    fn close_connection(&mut self, poll: &Poll, token: Token) -> Result<(), Error> {
        if let Some(conn) = self.get_connection_mut(token) {
            poll.registry().deregister(&mut conn.stream)?;
        } else {
            return Ok(());
        }
        self.connections.remove(&token);
        Ok(())
    }
}
