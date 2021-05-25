#![allow(dead_code)]
use crate::ftp::{RequestContextMutex, RequestType};
use mio::{event::Event, Events, Interest, Poll, Token};
use mio::{
    net::{TcpListener, TcpStream},
    Waker,
};
use std::error::Error;
use std::io::ErrorKind;
use std::sync::{Arc, Mutex};

// use crate::stats::program_information;

const SERVER: Token = Token(0);
const THREAD: Token = Token(2_147_483_647);

// pub fn convert_to_server(id: u64) -> u64 {
//     id | (1 << 63)
// }

// pub fn is_server(id: u64) -> bool {
//     id & (1 << 63) == 1
// }

// #[derive(Debug)]
// pub enum ActionType {
//     Readable,
//     Writable,
//     Disconnect,
// }

pub trait TCPImplementation {
    fn action_list(&mut self) -> Arc<Mutex<Vec<(Token, RequestContextMutex, Interest)>>>;

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
    fn write_connection(
        &mut self,
        poll: &Poll,
        waker: Arc<Waker>,
        event: &Event,
    ) -> Result<(), std::io::Error>;

    /// Read connection
    /// ## Behaviour
    /// * When returning an error that it's not `WouldBlock`, it will call `close_connection`
    /// * When returning an error that it's `NonBlocking` it will do nothing,
    /// * On OK it does nothing
    fn read_connection(
        &mut self,
        poll: &Poll,
        waker: Arc<Waker>,
        event: &Event,
    ) -> Result<(), std::io::Error>;

    /// Close connection handler
    fn close_connection(
        &mut self,
        poll: &Poll,
        id: Token,
        waker: &Arc<Waker>,
    ) -> Result<(), std::io::Error>;

    /// Function that will be called when the server needs a new id for the next connection
    fn next_id(&mut self) -> usize;
}

fn handle_request_type(
    request: &mut RequestContextMutex,
    poll: &Poll,
    interest: Interest,
    token: Token,
) -> Result<(), std::io::Error> {
    let mut r = request.lock().unwrap();
    match &mut r.request_type {
        RequestType::Closed(stream)
        | RequestType::CommandTransfer(stream, _, _, _)
        | RequestType::FileTransferActive(stream, _, _)
        | RequestType::FileTransferPassive(stream, _, _) => {
            if interest == Interest::AIO {
                println!("deregister");
            } else {
                let _ = poll.registry().deregister(stream);
                poll.registry().register(stream, token, interest)?;
            }
        }
        RequestType::PassiveModePort(stream, _) => {
            if interest == Interest::AIO {
                // poll.registry().deregister(stream)?;
            } else {
                poll.registry().register(stream, token, interest)?;
            }
        }
    }
    Ok(())
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
    // We need this so we can wake up the poll from another thread when we add new events
    let waker = Arc::new(Waker::new(poll.registry(), THREAD)?);
    loop {
        {
            let actions = tcp_implementation.action_list();
            let actions = actions.lock();
            if let Ok(mut actions) = actions {
                for (token, mut request, type_action) in actions.drain(..) {
                    handle_request_type(&mut request, &poll, type_action, token)?;
                }
            }
        }

        // Poll Mio for events, blocking until we get an event.
        poll.poll(&mut events, None)?;

        // Process each event.
        for event in events.iter() {
            if event.is_error() || event.is_read_closed() || event.is_write_closed() {
                let res = tcp_implementation.close_connection(&poll, event.token(), &waker);
                // If there was an error closing it means that the user doesn't want to close this connection yet
                if !res.is_err() {
                    continue;
                }
            }
            // We can use the token we previously provided to `register` to
            // determine for which socket the event is.
            match event.token() {
                SERVER => {
                    // If this is an event for the server, it means a connection
                    // is ready to be accepted.
                    loop {
                        match server.accept() {
                            Ok((stream, _)) => {
                                if let Err(_) = tcp_implementation.new_connection(
                                    SERVER,
                                    Token(id),
                                    &poll,
                                    stream,
                                ) {
                                    let _ = tcp_implementation.close_connection(
                                        &poll,
                                        Token(id),
                                        &waker,
                                    );
                                }
                                id = tcp_implementation.next_id();
                            }
                            _ => break,
                        }
                    }
                }
                THREAD => {
                    continue;
                }
                Token(_) => {
                    if event.is_writable() {
                        if let Err(err) =
                            tcp_implementation.write_connection(&poll, waker.clone(), event)
                        {
                            match err.kind() {
                                ErrorKind::WouldBlock => {
                                    continue;
                                }
                                _ => {
                                    let _ = tcp_implementation.close_connection(
                                        &poll,
                                        event.token(),
                                        &waker,
                                    );
                                }
                            }
                        }
                    } else if event.is_readable() {
                        if let Err(err) =
                            tcp_implementation.read_connection(&poll, waker.clone(), event)
                        {
                            match err.kind() {
                                ErrorKind::WouldBlock => {
                                    continue;
                                }
                                _ => {
                                    if let Err(err) = tcp_implementation.close_connection(
                                        &poll,
                                        event.token(),
                                        &waker,
                                    ) {
                                        println!(
                                            "something happened when closing a socket: {}",
                                            err
                                        );
                                    }
                                }
                            }
                        }
                    }
                    // if event.is_error() || (event.is_read_closed()) || (event.is_write_closed()) {
                    //     println!("{:?}", event);
                    //     let _ = tcp_implementation.close_connection(&poll, event.token(), &waker);
                    //     continue;
                    // }
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
