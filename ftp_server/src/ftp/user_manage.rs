extern crate serde_json;
// use serde_json::Value as JsonValue;
extern crate serde;
use serde::Deserialize;
extern crate chrono;

use std::{
    fs::{
        self,
        File
    },
    path::Path,
    collections::HashMap,
    error::Error,
    fs::OpenOptions,
    io::Write,
};


pub const USER_PATH: &'static str = "./etc/users.json";
pub const LOG_PATH: &'static str = "./var/ftpserver.log";


/// Structure that stores the user data of a connection
#[derive(Deserialize, Debug, Clone)]
pub struct User {
    passwd: String,
    chroot: String,
    uid: u16,

    #[serde(skip)]
    actual_dir: String,
}


impl User {

    pub fn change_dir (&mut self, new_dir: &str) -> Result<(), &'static str> {

        // we should use
        // std::env::set_current_dir("./root").unwrap();

        let mut temp_dir = String::new();

        if new_dir.starts_with("/") {
            temp_dir = ".".to_string() + new_dir;
        }

        if Path::new(&temp_dir).exists() {
            self.actual_dir = temp_dir;
            Ok(())
        } else {
            Err("Directony doesn't exist")
        }
    }


    pub fn has_passwd (&self, passwd: &str) -> bool {
        self.passwd == passwd
    }

    pub fn get_actual_dir (&self) -> &String {
        &self.actual_dir
    }

    pub fn get_chroot (&self) -> &String {
        &self.chroot
    }

    pub fn get_uid (&self) -> u16 {
        self.uid
    }

}


/// Structure that stores all users
/// User objects are stored in a HashMap, where the username is the key
/// to be able to give a reference to each connection when the server receives the `USER` command
#[derive(Debug)]
pub struct SystemUsers {
    config_path: String,
    users_data: HashMap<String, User>,
    log_file: File,
}


impl SystemUsers {

    pub fn load_data (filename: &str) -> Result<Self, Box<dyn Error>> {
        let content = fs::read_to_string(filename)?;
        let mut users_data: HashMap<String, User> = serde_json::from_str(&content)?;

        users_data.iter_mut()
            .for_each(|(_, user)| {
                user.actual_dir = user.chroot.clone();
            });

        let log_file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(LOG_PATH)?;

        Ok(Self {
            config_path: filename.to_string(),
            users_data,
            log_file,
        })
    }


    pub fn user_exists (&self, user_name: &str) -> bool {
        let time = chrono::offset::Local::now();
        writeln!(&self.log_file, "[{:?}] Looking for USER {}", time, user_name).unwrap();
        self.users_data
            .iter()
            .any(|(u, _)| u == user_name)
    }


    pub fn has_passwd (&self, user_name: &str, passwd: &str) -> bool {
        let time = chrono::offset::Local::now();
        writeln!(&self.log_file, "[{:?}] Looking for USER {}, PASS {}", time, user_name, passwd).unwrap();
        if let Some(user) = self.users_data.get(user_name) {
            &user.passwd == passwd
        } else {
            false
        }
    }


    pub fn get_user (&self, user_name: &str) -> Option<User> {
        if self.user_exists(user_name) {
            Some(self.users_data[user_name].clone())
        } else {
            None
        }
    }

}


// #[cfg(test)]
// mod system_users_test {

    // use super::SystemUsers;
    // const USER_PATH: &'static str = "./etc/users.json";

    // #[test]
    // fn check_exist () {
        // let user_list = SystemUsers::load_data(USER_PATH).unwrap();
        // assert!(user_list.user_exists("admin"));
    // }

    // #[test]
    // fn look_for_passwords () {
        // let user_list = SystemUsers::load_data(USER_PATH).unwrap();
        // assert!(user_list.has_passwd("admin", "admin"));
    // }
    

    // #[test]
    // fn concurrent_modif () {
        // // cargo t concurrent_modif -- --nocapture
        // use std::sync::{Arc, Mutex};
        // use std::thread;

        // let user_list = SystemUsers::load_data(USER_PATH).unwrap();
        // let user_list = Arc::new(Mutex::new(user_list));
        // let mut handles = vec![];
        // let users_names = ["admin", "root", "anonymous", "user", "marikong"];

        // for i in 0..5 {
            // let cloned = Arc::clone(&user_list);
            // let handle = thread::spawn(move|| {
                // let mut users_shared = cloned.lock().unwrap();
                // let user = users_shared.get_user(users_names[i]).unwrap();
                // // user.change_dir("/src").unwrap();
                // println!("{} - {:#?}", i, user);
            // });

            // handles.push(handle);
        // }

        // for handle in handles {
            // handle.join().unwrap();
        // }

        // println!("{:#?}", user_list);

    // }

// }
