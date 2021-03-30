use std::{convert::TryFrom, net::Ipv4Addr, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub enum Command<'a> {
    /// To initiate any data transfer in active mode, the client must send this command.
    /// The first 4 bytes is the host IPv4 addr. and the rest is the port number
    ///  h1,h2,h3,h4,p1,p2
    Port(Ipv4Addr, u16),

    /// Pointer to string, which indicates the desired folder path
    /// ## Cases
    /// * '/' | './' | None -> CurrentFolder
    List(&'a Path),
}

fn expects_byte(byte: u8, expected_byte: u8, msg: &'static str) -> Result<(), &'static str> {
    if byte != expected_byte {
        return Err(msg);
    }
    Ok(())
}

fn ascii_to_u8(buff: &[u8]) -> Option<u8> {
    if buff.len() > 3 {
        return None;
    }
    let mut n: u16 = 0;
    for byte in buff {
        if n > 0 {
            n *= 10;
        }
        let digit = (*byte as char).to_digit(10)?;
        n += digit as u16;
    }
    if n > std::u8::MAX as u16 {
        return None;
    }
    Some(n as u8)
}

impl<'a> TryFrom<&'a [u8]> for Command<'a> {
    type Error = &'static str;

    fn try_from(command: &'a [u8]) -> Result<Self, &'static str> {
        if command.len() <= 2 {
            return Err("Command is too short");
        }
        expects_byte(
            command[command.len() - 1],
            b'\n',
            "All commands should finish with \r\n",
        )?;
        expects_byte(
            command[command.len() - 2],
            b'\r',
            "All commands should finish with \r\n",
        )?;
        // For maximum performance, we are gonna use a trie of matches
        match command[0] {
            // Possible commands = LIST
            b'L' => {
                if command.len() <= 6 {
                    return Err("invalid command length");
                }
                if &command[1..4] != b"IST" {
                    return Err("Invalid command, maybe you meant: `LIST`?");
                }
                expects_byte(
                    command[4],
                    b' ',
                    "Expected space in between command and the rest.",
                )?;
                // -2 because we wanna skip \r\n
                let path_str = std::str::from_utf8(&command[5..command.len() - 2])
                    .map_err(|_| "expected utf8 string")?;
                let path = Path::new(path_str);
                Ok(Command::List(path))
            }

            b'P' => {
                if command.len() <= 6 {
                    return Err("invalid command length");
                }
                if &command[1..4] != b"ORT" {
                    return Err("Invalid command, maybe you meant: `PORT`?");
                }
                expects_byte(
                    command[4],
                    b' ',
                    "Expected space in between command and the rest.",
                )?;

                let mut ip_addr = [0u8; 4];
                let mut port = [0u8; 2];
                let mut byte_idx = 5;

                // Parse IP + port
                for i in 0..6 {
                    let prev = byte_idx;
                    while byte_idx < command.len() - 2 && command[byte_idx] != b',' {
                        byte_idx += 1;
                    }
                    if i >= 4 {
                        port[i - 4] =
                            ascii_to_u8(&command[prev..byte_idx]).ok_or("Invalid port number")?;
                    } else {
                        ip_addr[i] =
                            ascii_to_u8(&command[prev..byte_idx]).ok_or("Invalid IPv4 address")?;
                    }
                    byte_idx += 1;
                }

                // Check we reached the end of the command
                if byte_idx != command.len() - 1 {
                    return Err("Bad format of the `PORT` command");
                }

                // Try to get the IPv4
                let ip = Ipv4Addr::from(ip_addr);

                // This is the formula for getting the port number
                let port: u16 = port[0] as u16 * 256 + port[1] as u16;
                Ok(Command::Port(ip, port))
            }

            _ => Err("invalid command"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Command;
    use std::{convert::TryFrom, net::Ipv4Addr, path::Path};

    #[test]
    fn check_command_parsing_works() {
        let tests = [
            (
                "LIST ./test/test/test1.txt\r\n".as_bytes(),
                Command::List(Path::new("./test/test/test1.txt")),
                true,
            ),
            (
                "PORT 0,0,0,0,0,20\r\n".as_bytes(),
                Command::Port(Ipv4Addr::new(0, 0, 0, 0), 20),
                true,
            ),
            (
                "PORT 255,255,100,100,40,20\r\n".as_bytes(),
                Command::Port(Ipv4Addr::new(255, 255, 100, 100), 40 * 256 + 20),
                true,
            ),
            (
                "PORT 1,253,0,20,40,200\r\n".as_bytes(),
                Command::Port(Ipv4Addr::new(1, 253, 0, 20), 40 * 256 + 200),
                true,
            ),
        ];
        for test in tests.iter() {
            let (command_buff, expected_path, should_be_equal) = test;
            let command_try = Command::try_from(&command_buff[..]);
            if let Err(msg) = command_try {
                panic!(msg);
            }
            let command = command_try.unwrap();
            assert_eq!(&command == expected_path, *should_be_equal);
        }
    }
}
