use std::{convert::TryFrom, path::Path};

#[derive(Clone, Debug, PartialEq)]
pub enum Command<'a> {
    /// To initiate any data transfer in active mode, the client must send this command.
    /// The first 4 bytes is the host IPv4 addr. and the rest is the port number
    Port(u32, u16),

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

impl<'a> TryFrom<&'a [u8]> for Command<'a> {
    type Error = &'static str;

    fn try_from(command: &'a [u8]) -> Result<Self, &'static str> {
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
                if &command[1..4] != b"IST" {
                    return Err("invalid command, maybe you meant: `LIST`");
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
            _ => Err("invalid command"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Command;
    use std::{convert::TryFrom, path::Path};

    #[test]
    fn check_command_parsing_works() {
        let tests = [(
            "LIST ./test/test/test1.txt\r\n".as_bytes(),
            Command::List(Path::new("./test/test/test1.txt")),
            true,
        )];
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
