use std::{
    fs::File,
    io::{stdout, Stdout, Write},
    sync::{Mutex, MutexGuard},
};

pub static mut DEBUG: bool = false;
pub static mut STDOUT_FILE: Option<Mutex<File>> = None;

// Don't call this in multithreaded environments!!
pub(crate) fn use_stdout(file: File) {
    // This is totally safe because we won't use this function concurrently
    unsafe {
        STDOUT_FILE = Some(Mutex::new(file));
    }
}

pub(crate) fn set_debug(debug: bool) {
    // This is totally safe because we won't use this function concurrently
    unsafe {
        DEBUG = debug;
    }
}

#[macro_export]
macro_rules! print_stdout {
    ($($arg:tt)*) => (
        {
            use crate::ftp::config::*;
            use std::io::prelude::*;
            unsafe {
                if DEBUG {
                    if let Err(e) = write!(&mut ::std::io::stdout(), "{}\n", format_args!($($arg)*)) {
                        panic!("Failed to write to stdout.\
                            \nOriginal error output: {}\
                            \nSecondary error writing to stderr: {}", format!($($arg)*), e);
                    }
                }
                if STDOUT_FILE.is_some() {
                    let mut m = STDOUT_FILE.as_mut().unwrap().lock().unwrap();
                    if let Err(e) = write!(m, "{}\n", format_args!($($arg)*)) {
                        panic!("Failed to write to stdout.\
                            \nOriginal error output: {}\
                            \nSecondary error writing to stderr: {}", format!($($arg)*), e);
                    }
                }
            }
        }
    )
}
