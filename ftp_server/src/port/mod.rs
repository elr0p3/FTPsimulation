use std::net::{TcpListener, TcpStream};

/// Returns a random port, if it's none it means that every port in the machine is taken.
pub fn get_random_port() -> Option<u16> {
    Some(
        TcpListener::bind("0.0.0.0:0")
            .ok()?
            .local_addr()
            .ok()?
            .port(),
    )
}

pub fn get_ftp_port_pair(port: u16) -> (u8, u8) {
    let first_part = port / 256;
    let second_part = port - first_part * 256;
    (first_part as u8, second_part as u8)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_random_port() {
        use super::get_random_port;
        get_random_port().expect("to work");
    }
}
