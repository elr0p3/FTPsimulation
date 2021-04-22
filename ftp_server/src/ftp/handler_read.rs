use super::{command::Command, response::Response, FileTransferType};
use super::{
    create_response, Action, ActionList, BufferToWrite, FTPServer, HashMutex, RequestContext,
    RequestContextMutex, RequestType, Token, ROOT,
};
use mio::{net::TcpStream, Interest, Waker};
use std::{
    convert::TryFrom,
    path::Path,
    sync::{Arc, Mutex},
};
use std::{
    fs::File,
    io::{ErrorKind, Read},
};
use std::{
    io::{Error, Write},
    net::Shutdown,
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

    fn handle_file_transfer_download(
        &mut self,
        ctx: &mut RequestContext,
        file: File,
    ) -> Result<(), Error> {
        match &mut ctx.request_type {
            RequestType::CommandTransfer(_, _, _) | RequestType::Closed(_) => {
                Err(Error::from(ErrorKind::NotFound))
            }
            RequestType::FileTransferPassive(_stream, ftt, _)
            | RequestType::FileTransferActive(_stream, ftt, _) => {
                *ftt = FileTransferType::FileDownload(file);
                Ok(())
            }
            RequestType::PassiveModePort(_, _) => Err(Error::from(ErrorKind::NotFound)),
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
    ) -> Result<Option<Box<dyn FnOnce(&mut RequestContext) + Send>>, Error> {
        match request_type {
            RequestType::CommandTransfer(stream, to_write, data_connection) => {
                let _ = stream.flush();
                // Initialize a big buffer
                let mut buff = [0; 10024];

                // Read thing into the buffer TODO Handle block in multithread
                let read = stream.read(&mut buff)?;

                println!(
                    "[HANDLE_READ] {} - {} bytes read",
                    self.connection_token.0, read
                );

                if !buff[0..read].contains(&b'\n') {
                    panic!("NOT INCLUDED JL");
                }

                // Testing condition
                if read >= buff.len() {
                    // Just close connection if the request is too big at the moment
                    return Err(Error::from(ErrorKind::Other));
                }

                // Translate to Command enum
                let possible_command = Command::try_from(&buff[..read]);

                // Check if it's a valid command
                if let Err(message) = possible_command {
                    println!(
                        "[HANDLE_READ] {} - User sent a bad command {}",
                        self.connection_token.0, message
                    );
                    to_write.reset(create_response(
                        Response::bad_sequence_of_commands(),
                        message,
                    ));
                    self.actions.push((
                        self.connection_token,
                        self.connection.clone(),
                        Interest::WRITABLE,
                    ));
                    return Ok(None);
                }

                // Get the command
                let command =
                    possible_command.expect("command parse is not an error, this is safe");

                match command {
                    Command::Quit => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));

                        to_write.reset_str("221 Service closing control connection.\r\n");
                        let conn = self.connection.clone();
                        to_write.callback_after_sending = Some(Box::new(move || {
                            let connection = conn.lock().unwrap();
                            if let RequestType::CommandTransfer(stream, _, _) =
                                &connection.request_type
                            {
                                let _ = stream.shutdown(Shutdown::Both);
                            }
                        }));
                        waker.wake()?;
                        return Ok(None);
                    }

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
                            return Ok(None);
                        }

                        // Example of parsing the path, later on we will need to build
                        // from here
                        let base = format!("{}/{}", ROOT, "username");
                        let root_path = Path::new(base.as_str()).canonicalize().unwrap();
                        let true_base = root_path.to_str().unwrap();
                        let total_path = root_path.join(path).canonicalize();
                        if let Ok(path) = total_path {
                            // println!("{:?}", path);
                            if !path.starts_with(true_base) {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, no access.",
                                ));
                                return Ok(None);
                            }
                            let file = File::open(path);
                            if let Err(_) = file {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, file not found.",
                                ));
                                return Ok(None);
                            }
                            let file = file.unwrap();
                            let mut connection_db = self.connection_db.lock().unwrap();
                            let token_data_conn = data_connection.take().unwrap();
                            let data_transfer_conn = connection_db.get_mut(&token_data_conn);
                            if data_transfer_conn.is_none() {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, no access.",
                                ));
                                return Ok(None);
                            }
                            let data_transfer_conn = data_transfer_conn.unwrap().clone();
                            // Drop mutex because we are gonna do more stuff
                            drop(connection_db);
                            let mut data_transfer_conn_mutex = data_transfer_conn.lock().unwrap();
                            if let Err(_) = self
                                .handle_file_transfer_download(&mut data_transfer_conn_mutex, file)
                            {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, file not found.",
                                ));
                            } else {
                                to_write.reset(create_response(
                                    Response::file_status_okay(),
                                    "File download starts!",
                                ));
                                let ctx = data_transfer_conn.clone();
                                let cb = move || {
                                    actions.lock().unwrap().push((
                                        token_data_conn,
                                        ctx,
                                        Interest::WRITABLE,
                                    ))
                                };
                                to_write.callback_after_sending = Some(Box::new(cb));
                            }
                        } else {
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, file not found.",
                            ));
                            return Ok(None);
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
                            return Ok(None);
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
                            return Ok(None);
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
                        return Ok(None);
                    }

                    // Test for when the user is logging in
                    _ => {
                        let username = "gabivlj";
                        // todo checking
                        let user_id = 3;
                        return Ok(Some(Box::new(move |mut ctx| {
                            ctx.user_id = Some(user_id);
                            ctx.loged = false;
                        })));
                    }
                }

                Ok(None)
            }

            RequestType::PassiveModePort(listener, command_conn_ref) => {
                // Accept file connection
                let (stream, _addr) = listener.accept()?;

                // Get the token for the connection
                let token_for_connection = Token(next_id);

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

                connection_db.remove(&self.connection_token);
                let cmd_connection = connection_db.get_mut(command_conn_ref);

                if let Some(cmd_connection) = cmd_connection {
                    let command_conn_arc = cmd_connection.clone();
                    if let RequestType::CommandTransfer(_stream, buff, f) =
                        &mut cmd_connection.lock().unwrap().request_type
                    {
                        *f = Some(Token(next_id));
                        buff.reset(create_response(Response::command_okay(), "Command okay."));
                        self.actions
                            .push((*command_conn_ref, command_conn_arc, Interest::WRITABLE))
                    }
                } else {
                    return Ok(None);
                }
                connection_db.insert(token_for_connection, shared_request_ctx.clone());
                // Remove the listener (won't accept more connections)

                Ok(None)
            }

            _ => unimplemented!("Unimplemented Request type"),
        }
    }
}
