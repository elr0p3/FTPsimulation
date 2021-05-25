use serde::{Deserialize, Serialize};

use std::{
    collections::HashMap,
    error::Error,
    fs::OpenOptions,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

pub const USER_PATH: &'static str = "./etc/users.json";
pub const LOG_PATH: &'static str = "./var/ftpserver.log";

/// Structure that stores the user data of a connection
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    passwd: String,
    chroot: String,
    uid: u16,

    #[serde(skip)]
    actual_dir: String,
}

impl User {
    pub fn create_dir(&self) {
        let _ = fs::create_dir(&self.chroot);
    }

    pub fn new(username: &str, passwd: &str, uid: u16) -> Self {
        let _ = fs::create_dir(format!("./root/{}", username));
        let chroot = "./root/".to_string() + username;
        Self {
            passwd: passwd.to_string(),
            chroot: chroot.clone(),
            uid,
            actual_dir: "./".to_string(),
        }
    }

    pub fn are_equal_paths(&mut self, path: &str) -> bool {
        return self.total_path() == Path::new(path).canonicalize().unwrap();
    }

    /// Same behaviour as change_dir but returning the expected path
    pub fn new_dir<'a>(
        chroot: &'a str,
        actual_dir: &str,
        new_dir: &'a Path,
    ) -> Result<String, &'static str> {
        // Get root
        let root = Path::new(chroot);
        // Get total path root
        let expected_root = root.canonicalize().unwrap();
        // Path buffer to build directory
        let mut path_buf: PathBuf = PathBuf::new();
        path_buf.push(actual_dir);
        path_buf.push(new_dir);
        // Final path making sure it's not absolute
        let final_path = format!("./{}", path_buf.to_str().unwrap());
        // Join with root
        let path = root.join(final_path.clone());
        println!("{:?}", path);
        // Get total path
        let total_path = path.canonicalize().map_err(|_| "Directory not found")?;
        // Check if it's valid (doesn't exit the chroot)
        let valid_dir = total_path.starts_with(&expected_root);
        if valid_dir {
            let p = total_path.to_str().unwrap();
            Ok(p.to_string())
        } else {
            Err("Invalid directory")
        }
    }

    pub fn change_dir<P: AsRef<Path>>(&mut self, new_dir: P) -> Result<(), &'static str> {
        // Get root
        let root = Path::new(&self.chroot);
        // Get total path root
        let expected_root = root.canonicalize().unwrap();
        // Path buffer to build directory
        let mut path_buf: PathBuf = PathBuf::new();
        path_buf.push(&self.actual_dir);
        path_buf.push(new_dir);
        // Final path making sure it's not absolute
        let final_path = format!("./{}", path_buf.to_str().unwrap());
        // Join with root
        let path = root.join(final_path.clone());
        // Get total path
        let total_path = path.canonicalize().map_err(|_| "Directory not found")?;
        // Check if it's valid (doesn't exit the chroot)
        let valid_dir = total_path.starts_with(&expected_root);
        if valid_dir {
            // If it's actually valid set it to actual directory
            self.actual_dir = final_path;
            Ok(())
        } else {
            Err("Invalid directory")
        }
    }

    pub fn has_passwd(&self, passwd: &str) -> bool {
        self.passwd == passwd
    }

    pub fn get_actual_dir(&self) -> &String {
        &self.actual_dir
    }

    pub fn get_chroot(&self) -> &String {
        &self.chroot
    }

    // Gets the total path of the user (in the system)
    pub fn total_path(&self) -> PathBuf {
        Path::new(&self.chroot)
            .join(&self.actual_dir)
            .canonicalize()
            .unwrap()
    }

    pub fn total_path_and_decano(&self) -> String {
        let t = self.total_path().to_str().unwrap().replacen(
            Path::new(&self.chroot)
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap(),
            "",
            1,
        );
        if t == "" {
            return String::from("/");
        }
        t
    }

    // Gets the total path of the user (in the system)
    pub fn total_path_non_canon(&self) -> String {
        Path::new(&self.chroot)
            .join(&self.actual_dir)
            .to_str()
            .unwrap()
            .to_string()
    }

    pub fn get_uid(&self) -> u16 {
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
    pub fn load_data(filename: &str) -> Result<Self, Box<dyn Error>> {
        let content = fs::read_to_string(filename)?;
        let mut users_data: HashMap<String, User> = serde_json::from_str(&content)?;

        users_data.iter_mut().for_each(|(_, user)| {
            user.actual_dir = "./".to_string();
            user.create_dir();
        });

        let log_file = OpenOptions::new().write(true).append(true).open(LOG_PATH)?;

        Ok(Self {
            config_path: filename.to_string(),
            users_data,
            log_file,
        })
    }

    pub fn user_exists(&self, user_name: &str) -> bool {
        let time = chrono::offset::Local::now();
        writeln!(
            &self.log_file,
            "[{:?}] Looking for USER {}",
            time, user_name
        )
        .unwrap();
        self.users_data.iter().any(|(u, _)| u == user_name)
    }

    pub fn has_passwd(&self, user_name: &str, passwd: &str) -> bool {
        let time = chrono::offset::Local::now();
        writeln!(
            &self.log_file,
            "[{:?}] Looking for USER {}, PASS {}",
            time, user_name, passwd
        )
        .unwrap();
        if let Some(user) = self.users_data.get(user_name) {
            &user.passwd == passwd
        } else {
            false
        }
    }

    pub fn get_user<'a>(&'a self, user_name: &str) -> Option<&'a User> {
        if self.user_exists(user_name) {
            Some(&self.users_data[user_name])
        } else {
            None
        }
    }

    pub fn get_user_mut<'a>(&'a mut self, user_name: &str) -> Option<&'a mut User> {
        if self.user_exists(user_name) {
            Some(self.users_data.get_mut(user_name).unwrap())
        } else {
            None
        }
    }

    pub fn get_user_clone(&self, user_name: &str) -> Option<User> {
        if self.user_exists(user_name) {
            Some(self.users_data[user_name].clone())
        } else {
            None
        }
    }

    pub fn create_user(&mut self, user_name: &str, passwd: &str) -> Result<(), &'static str> {
        let time = chrono::offset::Local::now();
        writeln!(
            &self.log_file,
            "[{:?}] Looking for USER {}, PASS *****",
            time, user_name
        )
        .unwrap();
        if let Some(_) = self.users_data.get(user_name) {
            writeln!(
                &self.log_file,
                "[{:?}] User '{}' already exists",
                time, user_name
            )
            .unwrap();
            return Err("User already exists");
        }

        let mut uid: u16 = 0;

        for (_, user) in self.users_data.iter() {
            if uid <= user.get_uid() {
                uid = user.get_uid() + 1;
            }
        }

        let user = User::new(user_name, passwd, uid);
        let _ = fs::create_dir(&user.get_actual_dir());
        self.users_data.insert(user_name.to_string(), user);
        self.serialize_users().unwrap();

        writeln!(
            &self.log_file,
            "[{:?}] User '{}' has been created and stored",
            time, user_name
        )
        .unwrap();

        Ok(())
    }

    pub fn delete_user(&mut self, user_name: &str, passwd: &str) -> Result<User, &'static str> {
        let time = chrono::offset::Local::now();
        writeln!(
            &self.log_file,
            "[{:?}] Looking for USER {}, PASS *****",
            time, user_name
        )
        .unwrap();

        if let Some(user_content) = self.users_data.get(user_name) {
            if !user_content.has_passwd(passwd) {
                writeln!(
                    &self.log_file,
                    "[{:?}] Invalid password for USER {}",
                    time, user_name
                )
                .unwrap();

                return Err("Invalid password");
            }

            writeln!(
                &self.log_file,
                "[{:?}] User '{}' has been deleted",
                time, user_name
            )
            .unwrap();

            let user = self.users_data.remove(user_name).unwrap();
            self.serialize_users().unwrap();
            Ok(user)
        } else {
            writeln!(
                &self.log_file,
                "[{:?}] User '{}' do not exists",
                time, user_name
            )
            .unwrap();

            Err("User do not exists")
        }
    }

    fn serialize_users(&self) -> Result<(), Box<dyn Error>> {
        let user_data = serde_json::to_string_pretty(&self.users_data)?;
        fs::write(USER_PATH, &user_data)?;
        Ok(())
    }
}

#[cfg(test)]
mod system_users_test {

    use super::{SystemUsers, USER_PATH};
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

    #[test]
    fn check_paths() {
        let new_user_name = "qwerty2";
        let new_user_passwd = new_user_name;
        let mut sys_users = SystemUsers::load_data(USER_PATH).unwrap();
        let create = sys_users.create_user(new_user_name, new_user_passwd);
        assert!(create.is_ok());
        let user = sys_users.get_user_mut(new_user_name).expect("to work");
        user.change_dir("./thing")
            .expect_err("Expect this an error");
        assert!(user.are_equal_paths("./home/qwerty2"));
        user.change_dir("..")
            .expect_err("Expected this to be an error");
        assert!(user.are_equal_paths("./home/qwerty2"));
        user.change_dir("/thing3/thing4")
            .expect("expect this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing3/thing4"));
        user.change_dir("../").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing3"));
        user.change_dir("..").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2"));
        user.change_dir("./thing3").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing3"));
        user.change_dir("./thing4").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing3/thing4"));
        user.change_dir("/thing2").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing2"));
        user.change_dir("/thing3/thing4")
            .expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2/thing3/thing4"));
        user.change_dir("/").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2"));
        assert!(user.are_equal_paths("./home/qwerty2//././././././//./"));
        user.change_dir("./thing3").expect("Expected this to be ok");
        assert!(user.are_equal_paths("./home/qwerty2//././././././//./thing3/thing4/.."));
        assert!(!user.are_equal_paths("./home/qwerty2//././././././//./thing3/thing4/./."));
    }

    // cargo t create_delete_user -- --nocapture
    #[test]
    fn create_delete_user() {
        let new_user_name = "qwerty";
        let new_user_passwd = new_user_name;
        let mut sys_users = SystemUsers::load_data(USER_PATH).unwrap();

        let created = sys_users.create_user(new_user_name, new_user_passwd);
        assert!(created.is_ok());

        let fail_create = sys_users.create_user(new_user_name, new_user_passwd);
        assert!(fail_create.is_err());

        let deleted = sys_users.delete_user(new_user_name, new_user_passwd);
        assert!(deleted.is_ok());

        let fail_delete = sys_users.delete_user("root", "1234");
        assert!(fail_delete.is_err());

        let fail_delete = sys_users.delete_user(new_user_name, new_user_passwd);
        assert!(fail_delete.is_err());
    }
}
