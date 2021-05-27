pub mod ftp;
pub mod port;
pub mod system;
pub mod tcp;
use std::fs::OpenOptions;

use clap::{App, Arg};
fn main() {
    let matches = App::new("FTP Server")
        .version("1.0")
        .author("Gabriel Villalonga @gabivlj\nRodrigo Pereira @_\nDaniel Gracia @DaniGMX")
        .about("Simple to use FTP server, with multithreading and non-blocking behaviour in mind for maximum concurrent file transfers for multiple users.")
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .default_value("8080")
                .help("Set port")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("capacity")
                .help("Sets maximum concurrent connections")
                .short("c")
                .long("capacity")
                .value_name("CAPACITY")
                .default_value("500"),
        )               
        .arg(
            Arg::with_name("debug")
                .help("If it should write to stdout the logs")
                .short("d")
                .long("debug")
                .value_name("DEBUG")
                .default_value("true"),
        )
        .arg(
            Arg::with_name("log_file")
                .help("If it should write to the specified file the logs, don't pass anything to not use a log file.")
                .short("l")
                .long("log_file")
                .value_name("LOG_FILE")
                .default_value("--none--"),
        )
        .get_matches();
    let debug: bool = matches.value_of("debug").unwrap().parse().unwrap(); 
    let log_file: &str = matches.value_of("log_file").unwrap(); 
    if log_file != "--none--" {
        ftp::config::use_stdout(OpenOptions::new().write(true).open(log_file).expect("Error with log file"));
    }
    ftp::config::set_debug(debug);
    let port = matches.value_of("port").unwrap();
    let capacity: usize = matches.value_of("capacity").unwrap().parse().unwrap();
    let ip = format!("0.0.0.0:{}", port);
    let mut ftp_server = ftp::FTPServer::with_connection_capacity(capacity);
    tcp::create_server(ip.as_str(), &mut ftp_server).expect("server returned an error");
}
