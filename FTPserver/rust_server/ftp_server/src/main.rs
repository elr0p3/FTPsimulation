pub mod ftp;
pub mod tcp;

fn main() {
    let mut ftp_server = ftp::FTPServer::new();
    tcp::create_server("127.0.0.1:8080", &mut ftp_server).expect("server returned an error");
}
