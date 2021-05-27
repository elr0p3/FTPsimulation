pub mod ftp;
pub mod port;
pub mod system;
pub mod tcp;
use clap::{App, Arg, SubCommand};
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
        .get_matches();
    let port = matches.value_of("port").unwrap();
    let capacity: usize = matches.value_of("capacity").unwrap().parse().unwrap();
    let ip = format!("0.0.0.0:{}", port);
    let mut ftp_server = ftp::FTPServer::with_connection_capacity(capacity);
    tcp::create_server(ip.as_str(), &mut ftp_server).expect("server returned an error");
}
