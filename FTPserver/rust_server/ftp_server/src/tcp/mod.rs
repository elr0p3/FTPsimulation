#![allow(dead_code)]
use mio::net::{TcpListener, TcpStream};
use mio::{event::Event, Events, Interest, Poll, Token};
use std::error::Error;
use std::io::ErrorKind;

// use crate::stats::program_information;

const SERVER: Token = Token(0);

// pub fn convert_to_server(id: u64) -> u64 {
//     id | (1 << 63)
// }

// pub fn is_server(id: u64) -> bool {
//     id & (1 << 63) == 1
// }

pub trait TCPImplementation {
    fn new_connection(
        &mut self,
        token_server: Token,
        token: Token,
        poll: &Poll,
        stream: TcpStream,
    ) -> Result<(), std::io::Error>;

    /// Write connection
    /// ## Behaviour
    /// * When returning an error that it's not `WouldBlock`, it will call `close_connection`.
    /// * When returning an error that it's `NonBlocking` it will do nothing.
    /// * On OK it does nothing
    fn write_connection(&mut self, poll: &Poll, event: &Event) -> Result<(), std::io::Error>;

    /// Read connection
    /// ## Behaviour
    /// * When returning an error that it's not `WouldBlock`, it will call `close_connection`
    /// * When returning an error that it's `NonBlocking` it will do nothing,
    /// * On OK it does nothing
    fn read_connection(&mut self, poll: &Poll, event: &Event) -> Result<(), std::io::Error>;

    /// Close connection handler
    fn close_connection(&mut self, poll: &Poll, id: Token) -> Result<(), std::io::Error>;

    /// Function that will be called when the server needs a new id for the next connection
    fn next_id(&mut self) -> usize;
}

pub fn create_server<T: AsRef<str>>(
    addr: T,
    tcp_implementation: &mut dyn TCPImplementation,
) -> Result<(), Box<dyn Error>> {
    // Create a poll instance.
    let mut poll = Poll::new()?;
    // Create storage for events.
    let mut events = Events::with_capacity(128);
    // Unique id for a connection
    let mut id = tcp_implementation.next_id();
    // Setup the server socket.
    let addr = addr.as_ref().parse()?;
    // Main server listener, even though you can create more bindings
    let mut server = TcpListener::bind(addr)?;
    // Start listening for incoming connections.
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;

    loop {
        // Poll Mio for events, blocking until we get an event.
        poll.poll(&mut events, None)?;

        // Process each event.
        for event in events.iter() {
            // We can use the token we previously provided to `register` to
            // determine for which socket the event is.
            match event.token() {
                SERVER => {
                    // If this is an event for the server, it means a connection
                    // is ready to be accepted.
                    let (stream, _addr) = server.accept()?;
                    if let Err(_) =
                        tcp_implementation.new_connection(SERVER, Token(id), &poll, stream)
                    {
                        tcp_implementation.close_connection(&poll, Token(id))?;
                    }
                    id = tcp_implementation.next_id();
                }
                Token(_) => {
                    if event.is_read_closed() {
                        if let Err(error) =
                            tcp_implementation.close_connection(&poll, event.token())
                        {
                            println!("message when closing a connection: {}", error);
                        }
                    } else if event.is_writable() {
                        if let Err(err) = tcp_implementation.write_connection(&poll, event) {
                            match err.kind() {
                                ErrorKind::WouldBlock => {
                                    continue;
                                }
                                _ => {
                                    tcp_implementation.close_connection(&poll, event.token())?;
                                }
                            }
                        }
                    } else if event.is_readable() {
                        if let Err(err) = tcp_implementation.read_connection(&poll, event) {
                            match err.kind() {
                                ErrorKind::WouldBlock => {
                                    continue;
                                }
                                _ => {
                                    if let Err(err) =
                                        tcp_implementation.close_connection(&poll, event.token())
                                    {
                                        println!(
                                            "something happened when closing a socket: {}",
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Run those callbacks depending on the result
/// Passes the `Ok` `result` on the `if_ok`
/// callback and the `Err` `result` on the `if_err` callback
fn do_callbacks<'a, T: 'static, E: 'static, OF, EF>(
    mut result: &'a mut Result<T, E>,
    mut if_ok: OF,
    mut if_err: EF,
) -> Result<(), ()>
where
    OF: FnMut(&mut T) -> Result<(), ()>,
    EF: FnMut(&mut E) -> Result<(), ()>,
{
    if let Ok(result) = &mut result {
        if_ok(result)
    } else if let Err(err) = &mut result {
        if_err(err)
    } else {
        unreachable!()
    }
}
