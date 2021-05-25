use super::{command::Command, response::Response, FileTransferType};
use super::{
    create_response, Action, ActionList, BufferToWrite, HashMutex, RequestContext,
    RequestContextMutex, RequestType, Token, ROOT,
};
use crate::system;
use crate::port::{get_ftp_port_pair, get_random_port};
use mio::{net::TcpListener, net::TcpStream, Interest, Waker};
use std::fs;
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
use user_manage::{SystemUsers, User};

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

    users_db: Arc<Mutex<SystemUsers>>,

    user_id: Option<String>,

    loged: bool,
}

pub enum ErrorTypeUser {
    PathNotFound,
    UserNotFound
}

impl HandlerRead {
    pub fn new(
        connection_token: Token,
        connection_db: HashMutex<Token, RequestContextMutex>,
        connection: RequestContextMutex,
        users_db: Arc<Mutex<SystemUsers>>,
        user_id: Option<String>,
        loged: bool,
    ) -> Self {
        Self {
            connection_token,
            connection_db,
            actions: Vec::new(),
            connection,
            users_db,
            user_id,
            loged,
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

    fn handle_file_transfer_upload(
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
                *ftt = FileTransferType::FileUpload(file, None);
                Ok(())
            }
            RequestType::PassiveModePort(_, _) => Err(Error::from(ErrorKind::NotFound)),
        }
    }

    pub fn get_user_path(&self) -> Option<String> {
        let user_id = self.user_id.as_ref().unwrap();
        let db = self.users_db.lock().unwrap();
        let user = db.get_user(user_id)?;        
        Some(user.get_chroot().to_string())
    } 

    pub fn get_user_path_non_canon(&self) -> String {
        let user_id = self.user_id.as_ref().unwrap();
        let db = self.users_db.lock().unwrap();
        let user = db.get_user(user_id).unwrap();        
        user.total_path_and_decano().to_string()
    } 

    pub fn handle_user_path(&self, path: &Path) -> Result<String, ErrorTypeUser> {
        let user_id = self.user_id.as_ref().unwrap();
        let db = self.users_db.lock().unwrap();
        let user = db.get_user(user_id);
        if let None = user {
            return Err(ErrorTypeUser::UserNotFound);
        }
        let user = user.unwrap();       
        User::new_dir(&&user.get_chroot(), &&user.get_actual_dir(), path).map_err(|_| ErrorTypeUser::PathNotFound)        
    }

    /// This function handles the read of the `request_type`,
    /// Will use `actions` for cloning its `Arc`, not for adquiring it
    /// `next_id` is assumed to be used, so the caller should provide always the next id
    /// Will return a possible callback that should be called when dropping the mutex_lock of the passed `request_type`
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

                if read == 0 {
                    self.actions.push((
                        self.connection_token,
                        self.connection.clone(),
                        Interest::READABLE,
                    ));
                    stream.shutdown(Shutdown::Both)?;
                    return Ok(None);
                }

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

                if command.is_auth_command() && (self.user_id.is_none() || !self.loged) {
                    self.actions.push((
                        self.connection_token,
                        self.connection.clone(),
                        Interest::WRITABLE,
                    ));
                    to_write.reset_str("531 Unauthorized.\r\n");
                    return Ok(None);
                }

                match command {
                    Command::CurrentDirectory => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        let users_db = self.users_db.lock().unwrap();
                        let user = users_db.get_user_clone(self.user_id.as_ref().unwrap()).unwrap();
                        drop(users_db);
                        to_write.reset(
                            create_response(Response::directory_action_okay(), &user.total_path_and_decano())
                        );
                    }

                    Command::ChangeDirectory(dir) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        let mut users_db = self.users_db.lock().unwrap();
                        let user = users_db.get_user_mut(self.user_id.as_ref().unwrap()).unwrap();
                        let result = user.change_dir(dir);
                        drop(user);
                        drop(users_db);
                        if result.is_err() {
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, file not found.",
                            ));
                            return Ok(None);
                        } 
                        to_write.reset(create_response(
                            Response::file_action_okay(),
                            "Requested file action okay, completed.",
                        ));
                        return Ok(None);
                    }

                    Command::Passive => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        let random_port = get_random_port();
                        if let Some(port) = random_port {
                            let tcp_listener =
                                TcpListener::bind(format!("0.0.0.0:{}", port).parse().unwrap())
                                    .expect("port to be init");
                            let mut db = self.connection_db.lock().unwrap();
                            let arc = Arc::new(Mutex::new(RequestContext::new(
                                RequestType::PassiveModePort(tcp_listener, self.connection_token),
                            )));
                            db.insert(Token(next_id), arc.clone());
                            self.actions.push((Token(next_id), arc, Interest::READABLE));
                            let (first_part, second_part) = get_ftp_port_pair(port);
                            to_write.reset_str(
                                format!(
                                    "227 Entering Passive Mode (0,0,0,0,{},{})\r\n",
                                    first_part, second_part
                                )
                                .as_str(),
                            );
                            return Ok(None);
                        }
                        to_write.reset_str("541 All ports are taken.\r\n");
                        return Ok(None);
                    }

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
                        return Ok(None);
                    }

                    Command::Password(pwd) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        let mut db = self.users_db.lock().unwrap();                        
                        if let Some(user_id) = &self.user_id {
                            if !db.user_exists(&user_id) {
                                let user = db.create_user(&user_id, pwd);
                                if user.is_err() {
                                    to_write.reset_str("530 Not logged in.\r\n");
                                    return Ok(None);
                                }
                                to_write.reset(create_response(
                                    Response::login_success(),
                                    "User logged in, proceed.",
                                ));
                                return Ok(Some(Box::new(|ctx| {
                                    ctx.loged = true;
                                })));
                            }
                            if db.has_passwd(user_id, pwd) {
                                to_write.reset(create_response(
                                    Response::login_success(),
                                    "User logged in, proceed.",
                                ));
                                return Ok(Some(Box::new(move |ctx| {                                
                                    ctx.loged = true;
                                })));
                            }
                            to_write.reset_str("530 Not logged in.\r\n");
                            return Ok(None);
                        }
                        to_write.reset_str("530 Not logged in.\r\n");
                        return Ok(None);
                    }

                    Command::User(username) => {                        
                        println!(
                            "[HANDLE_READ] {} - New user {}",
                            self.connection_token.0, username
                        );
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        to_write.reset(create_response(
                            Response::username_okay(),
                            "User name okay, need password.",
                        ));
                        let username = username.to_string();
                        return Ok(Some(Box::new(move |ctx| {
                            ctx.user_id = Some(username);
                            ctx.loged = false;
                        })));
                    }

                    Command::Delete(path) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        if let Ok(path) = self.handle_user_path(path) {
                            let result = fs::remove_file(path);
                            if let Err(_err) = result {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, file not found.",
                                ));
                                return Ok(None);
                            } 
                            to_write.reset(create_response(
                                Response::file_action_okay(),
                                "Requested file action okay, completed.",
                            ));
                            return Ok(None);
                        }  else { 
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, file not found.",
                            ));
                            return Ok(None);
                        }
                    }

                    Command::RemoveDirectory(directory) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        if let Ok(path) = self.handle_user_path(directory) {
                            let result = fs::remove_dir_all(path);
                            if let Err(_err) = result {
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, file not found.",
                                ));
                                return Ok(None);
                            } 
                            to_write.reset(create_response(
                                Response::file_action_okay(),
                                "Requested file action okay, completed.",
                            ));
                            return Ok(None);
                        }  else { 
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, file not found.",
                            ));
                            return Ok(None);
                        }
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
                        if let Ok(path) = self.handle_user_path(path) {                        
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

                    Command::Mkdir(path) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE
                        ));
                        let mut callback_error = || {
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, no access.",
                            ));
                        };
                        let path_user = self.get_user_path();
                        if let None = path_user {
                            callback_error();
                            return Ok(None);
                        }
                        let base = path_user.unwrap();
                        let root_path = Path::new(base.as_str()).canonicalize().unwrap();
                        let true_base = root_path.to_str().unwrap();
                        let mut path = path.to_path_buf();
                        let child = path.file_name().map(|el| el.to_str().unwrap().to_string()).clone();
                        path.pop();
                        let el = path.as_path();
                        let parent = if el == Path::new("/") { Path::new("./") } else { el };
                        if child.is_none() {
                            callback_error();
                            return Ok(None);
                        }
                        let child = child.unwrap();                                            
                        let parent = Path::new(self.get_user_path_non_canon().as_str()).join(parent);
                        let path = format!("./{}", parent.to_str().unwrap());                        
                        let total_path = root_path.join(path).canonicalize();
                        if let Ok(path) = total_path {
                            if !path.starts_with(true_base) {
                                callback_error();
                                return Ok(None);
                            }
                            let end_path = path.join(&child);
                            let result = std::fs::create_dir(end_path);
                            if result.is_err() {
                                callback_error();
                                return Ok(None);
                            }
                            let resp = format!("'{}' directory created.", child);
                            to_write.reset(create_response(
                                Response::directory_action_okay(),
                                resp.as_str(),
                            ));
                        } else {
                            callback_error();
                        }
                        return Ok(None);
                    }

                    Command::Store(path) => {
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::WRITABLE,
                        ));
                        let mut callback_error = || {
                            to_write.reset(create_response(
                                Response::file_unavailable(),
                                "Requested action not taken. File unavailable, no access.",
                            ));
                        };
                        if let None = data_connection {
                            callback_error();
                            return Ok(None);
                        }
                        let path_user = self.get_user_path();
                        if let None = path_user {
                            callback_error();
                            return Ok(None);
                        }
                        let base = path_user.unwrap();
                        let root_path = Path::new(base.as_str()).canonicalize();
                        if root_path.is_err() {                            
                            callback_error();
                            return Ok(None);
                        }                           
                        let root_path = root_path.unwrap();                 
                        let true_base = root_path.to_str().unwrap();
                        let mut path = path.to_path_buf();
                        let child = path.file_name().map(|el| el.to_str().unwrap().to_string()).clone();
                        path.pop();
                        let el = path.as_path();
                        let parent = if el == Path::new("/") { Path::new("./") } else { el };
                        if child.is_none() {
                            callback_error();
                            return Ok(None);
                        }
                        let child = child.unwrap();                                            
                        let parent = Path::new(self.get_user_path_non_canon().as_str()).join(parent);
                        let path = format!("./{}", parent.to_str().unwrap());
                        let total_path = root_path.join(path).canonicalize();
                        if let Ok(path) = total_path {
                            if !path.starts_with(true_base) {
                                callback_error();
                                return Ok(None);
                            }
                            let end_path = path.join(child);
                            let file_options = fs::OpenOptions::new()
                                .append(false)
                                .create(true)
                                .write(true)
                                .open(end_path.clone());
                            if let Ok(file) = file_options {
                                // Check that the file is really on a good position to exist
                                if end_path.canonicalize().is_err() {
                                    callback_error();
                                    return Ok(None);
                                }
                                let db = self.connection_db.lock().unwrap();
                                let token_data = data_connection.take().unwrap();
                                let conn = db.get(&token_data);
                                if let Some(conn) = conn {
                                    // Clone Arc because we must drop DB lock
                                    let conn = conn.clone();
                                    drop(db);
                                    let mut conn_lock = conn.lock().unwrap();
                                    if let Err(_) =
                                        self.handle_file_transfer_upload(&mut conn_lock, file)
                                    {
                                        callback_error();
                                        return Ok(None);
                                    }
                                    // HEH... I don't know but Rust doesn't get that this really needs to die here!
                                    drop(conn_lock);
                                    to_write.reset(create_response(
                                        Response::file_status_okay(),
                                        "File status okay; about to open data connection.",
                                    ));
                                    to_write.callback_after_sending =
                                        Some(Box::new(move || {                                         
                                            let mut actions = actions.lock().unwrap();
                                            actions.push((token_data, conn, Interest::READABLE));
                                        }));                            
                                    return Ok(None);
                                }
                            }
                            callback_error();
                            return Ok(None);
                        } 
                        callback_error();       
                        return Ok(None);                 
                    }

                    Command::List(path) => {
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
                            let res = self.handle_user_path(path);
                            if let Ok(path) = res {

                                let list = system::ls(path.as_str()).unwrap();
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
                                                list
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
                                to_write.reset(create_response(
                                    Response::file_unavailable(),
                                    "Requested action not taken. File unavailable, no access.",
                                ));
                            }
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
                        unimplemented!();
                        // let username = "gabivlj";
                        // // todo checking
                        // let user_id = 3;
                        // return Ok(Some(Box::new(move |mut ctx| {
                        //     ctx.user_id = Some(user_id);
                        //     ctx.loged = false;
                        // })));
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

                // Insert connection to the db
                connection_db.insert(token_for_connection, shared_request_ctx.clone());

                // Remove the port from the database
                connection_db.remove(&self.connection_token);
                
                // Get the comomand connection
                let cmd_connection = connection_db.get_mut(command_conn_ref);
                
                // Handle some
                if let Some(cmd_connection) = cmd_connection {
                    // Clone arc so we can push interest 
                    let command_conn_arc = cmd_connection.clone();
                    if let RequestType::CommandTransfer(_stream, buff, f) =
                        &mut cmd_connection.lock().unwrap().request_type
                    {
                        *f = Some(Token(next_id));
                        // *TODO Needs better response 
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

            RequestType::FileTransferActive(stream, type_connection, _data_conn_token)
            | RequestType::FileTransferPassive(stream, type_connection, _data_conn_token) => {
                println!("[HANDLE_READ] Yeah let's go");
                if let Ok(should_close) = self.handle_file_type(stream, type_connection) {
                    if should_close {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                } else {
                    let _ = stream.shutdown(Shutdown::Both);
                }
                return Ok(None);
            }

            _ => unimplemented!("Unimplemented Request type"),
        }
    }

    /// Returns true and Ok if it finished transfering, returns false and Ok if it needs more reads, returns Error if there is an error and needs shutdown
    fn handle_file_type(
        &mut self,
        stream: &mut TcpStream,
        transfer_type: &mut FileTransferType,
    ) -> Result<bool, ()> {
        match transfer_type {
            FileTransferType::FileUpload(file, possible_response) => {
                println!(
                    "[HANDLE_FILE_TYPE] {} - Reading from file transfer...",
                    self.connection_token.0
                );
                let mut buff = [0; 10024];
                let read_result = stream.read(&mut buff);
                self.actions.push((
                    self.connection_token,
                    self.connection.clone(),
                    Interest::READABLE,
                ));
                if let Ok(read_bytes) = read_result {                   
                    if read_bytes == 0 {
                        *possible_response = 
                        Some(create_response(
                            Response::success_uploading_file(), 
                            "Closing data connection. Requested file action successful (for example, file transfer or file abort)."
                        ));             
                        return Ok(true);
                    }
                    let err = file.write(&buff[..read_bytes]);
                    if err.is_err() {
                        println!(
                            "[HANDLE_FILE_TYPE] {} - Error writing to file {}...",
                            self.connection_token.0,
                            err.unwrap_err()
                        );
                        return Err(());
                    }                                      
                    println!(
                        "[HANDLE_FILE_TYPE] {} - Successfully read...",
                        self.connection_token.0
                    );
                } else if let Err(err) = read_result {
                    if err.kind() == ErrorKind::WouldBlock {
                        println!(
                            "[HANDLE_FILE_TYPE] {} - Would block...",
                            self.connection_token.0
                        );                        
                        self.actions.push((
                            self.connection_token,
                            self.connection.clone(),
                            Interest::READABLE,
                        ));
                        return Ok(false);
                    }                    
                    *possible_response = Some(b"451 Requested action aborted: local error in processing.\r\n".to_vec());
                    println!(
                        "[HANDLE_FILE_TYPE] {} - Error Reading File: {}...",
                        self.connection_token.0, err
                    );
                    // NOTE Thinking about doing file cleanup?
                    return Err(());
                }
                Ok(false)
            }
            _ => Err(()),
        }
    }
}
