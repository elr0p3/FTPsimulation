use super::{command::Command, response::Response, FileTransferType};
use super::{
    create_response, Action, ActionList, BufferToWrite, HashMutex, RequestContext,
    RequestContextMutex, RequestType, Token, ROOT,
};
use mio::{net::TcpStream, Interest, Waker};
use std::io::Error;
use std::{
    convert::TryFrom,
    path::Path,
    sync::{Arc, Mutex},
};
use std::{
    fs::File,
    io::{ErrorKind, Read},
};

pub struct HandlerRead {
    /// The request context token
    pub connection_token: Token,

    /// Connections database reference
    pub connection_db: HashMutex<Token, RequestContextMutex>,

    /// Local actions list, this should be used externally after using `handle_read`    
    /// When called `handle_read` this should be filled with the desired interests that
    /// this read handle wants
    pub actions: Vec<Action>,

    /// Connection mutex
    /// ** Warning Internals: This mutex should never adquired inside `handle_read`, only be used for cloning the Arc
    connection: RequestContextMutex,
}

impl HandlerRead {
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

    /// This function handles the read of the `request_type`,
    /// Will use `actions` for cloning its `Arc`, not for adquiring it
    /// `next_id` is assumed to be used, so the caller should provide always the next id
    pub fn handle_read(
        &mut self,
        request_type: &mut RequestType,
        waker: &Arc<Waker>,
        actions: ActionList,
        next_id: usize,
    ) -> Result<(), Error> {
        match request_type {
            RequestType::CommandTransfer(stream, to_write, data_connection) => {
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

                // Translate to Command enum
                let possible_command = Command::try_from(&buff[..read]);

                // Check if it's a valid command
                if let Err(message) = possible_command {
                    println!("User sent a bad command: {}", message);
                    to_write.reset(create_response(
                        Response::bad_sequence_of_commands(),
                        message,
                    ));
                    self.actions.push((
                        self.connection_token,
                        self.connection.clone(),
                        Interest::WRITABLE,
                    ));
                    return Ok(());
                }

                // Get the command
                let command =
                    possible_command.expect("command parse is not an error, this is safe");

                match command {
                    Command::Retr(path) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        if let None = data_connection {
                            to_write.reset(create_response(
                                Response::bad_sequence_of_commands(),
                                "Bad sequence of commands.",
                            ));
                            return Ok(());
                        }
                        // Example of parsing the path, later on we will need to build
                        // from here
                        let base = format!("{}/{}", ROOT, "username");
                        let root_path = Path::new(base.as_str());
                        let total_path = root_path.join(path).canonicalize();
                        if let Ok(path) = total_path {
                            if !path.starts_with(base) {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, no access.",
                                ));
                                return Ok(());
                            }
                            let file = File::open(path);
                            if let Err(_) = file {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, file not found.",
                                ));
                                return Ok(());
                            }
                            let _file = file.unwrap();
                        } else {
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, file not found.",
                            ));
                            return Ok(());
                        }
                    }

                    Command::List(_path) => {
                        // Inform that we are interested in writing a command again
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));

                        // This means that the user hasn't opened a port or connected
                        if let None = data_connection {
                            to_write.reset(create_response(
                                Response::bad_sequence_of_commands(),
                                "Bad sequence of commands.",
                            ));
                            return Ok(());
                        }

                        // All okay, transition
                        to_write.reset(create_response(
                            Response::file_status_okay(),
                            "File status okay; about to open data connection.",
                        ));

                        // Now here we are gonna prepare the callback for when the
                        // transition command is sent
                        let data_connection = data_connection.unwrap();

                        // get the connection database
                        let mut connection_db = self.connection_db.lock().unwrap();

                        // Get the data transfer connection
                        let connection = connection_db.get_mut(&data_connection);

                        // Clone waker smart reference
                        let waker = waker.clone();

                        // Unwrap the connection
                        if let Some(connection) = connection {
                            // Clone the smart reference of this request context
                            let connection = connection.clone();
                            // Create a callback that captures everything it needs
                            let callback = move || {
                                // Lock the request context
                                let mut connection_m = connection.lock().unwrap();

                                // Remember that this is the data connection,
                                // fill the data to write
                                match &mut connection_m.request_type {
                                    RequestType::FileTransferPassive(_, ftt, _)
                                    | RequestType::FileTransferActive(_, ftt, _) => {
                                        *ftt = FileTransferType::Buffer(BufferToWrite::new(
                                            vec![1].repeat(1000),
                                        ));
                                    }
                                    _ => unimplemented!(),
                                }

                                // Now that we are free of the connection mutex, it's safe
                                // to add to the actions array
                                actions.lock().unwrap().push((
                                    data_connection,
                                    connection.clone(),
                                    Interest::WRITABLE,
                                ));

                                // wake the Poll
                                let _ = waker.wake();
                            };

                            // Set the callback
                            to_write.callback_after_sending = Some(Box::new(callback));
                        } else {
                            // Inform the user that we couldn't find the data connection
                            to_write.reset(create_response(
                                Response::cant_open_data_connection(),
                                "Can't open data connection.",
                            ));
                        }
                    }

                    // When this command is fired we should connect to the desired port by the user
                    Command::Port(ip, port) => {
                        // Clone the database conn reference
                        let map_conn = self.connection_db.clone();

                        let connection =
                            TcpStream::connect(format!("{}:{}", ip, port).parse().unwrap());

                        let mut connections = map_conn.lock().unwrap();

                        let command_connection = connections
                            .get_mut(&self.connection_token)
                            .expect("TODO handle this error");

                        // Tell the server that we want to write
                        self.actions.push((
                            self.connection_token,
                            command_connection.clone(),
                            Interest::WRITABLE,
                        ));

                        // Handle error where the connection is not opened by the client
                        if connection.is_err() {
                            to_write.reset(create_response(
                                Response::bad_sequence_of_commands(),
                                "Bad sequence of commands.",
                            ));
                            return Ok(());
                        }

                        // fill data connection token (so later on the request context command keeps a reference
                        // to the request context of the file transfer)
                        *data_connection = Some(Token(next_id));

                        to_write.reset(create_response(Response::command_okay(), "Command okay."));

                        drop(command_connection);

                        let connection = connection.unwrap();
                        let request_ctx = Arc::new(Mutex::new(RequestContext::new(
                            RequestType::FileTransferActive(
                                connection,
                                FileTransferType::Buffer(BufferToWrite::default()),
                                self.connection_token,
                            ),
                        )));
                        connections.insert(Token(next_id), request_ctx);
                        return Ok(());
                    }
                }

                Ok(())
            }

            RequestType::PassiveModePort(listener, command_conn_ref) => {
                // Accept file connection
                let (stream, _addr) = listener.accept()?;

                // Get the token for the connection
                let token_for_connection = Token(next_id);

                // Register the connection as writable/readable
                // TODO Note that we need to put in passivemodeport the field of which kind of connection is this
                // (Download, Upload, Just Buffer Transfer...)

                // Add the connection
                let mut connection_db = self.connection_db.lock().unwrap();
                let shared_request_ctx = Arc::new(Mutex::new(RequestContext::new(
                    RequestType::FileTransferPassive(
                        stream,
                        FileTransferType::Buffer(BufferToWrite::new(vec![1])),
                        *command_conn_ref,
                    ),
                )));

                connection_db.insert(token_for_connection, shared_request_ctx.clone());

                self.actions
                    .push((token_for_connection, shared_request_ctx, Interest::WRITABLE));

                // Remove the listener (won't accept more connections)
                connection_db.remove(&self.connection_token);

                // Just deregister
                self.actions
                    .push((Token(0), self.connection.clone(), Interest::AIO));
                Ok(())
            }

            _ => unimplemented!("Unimplemented Request type"),
        }
    }
}
