#[derive(Clone, Copy, Debug)]
pub struct Response(pub u16);

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CodeFirst {
    PositivePreliminary = 100,
    Positive = 200,
    PositiveIntermediate = 300,
    TransientNegativeCompletion = 400,
    PermanentNegativeCompletion = 500,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum CodeSecond {
    Syntax = 0,
    Information = 10,
    Connections = 20,
    AuthenticationAndAccounting = 30,
    Unspecified = 40,
    FileSystem = 50,
}

impl Response {
    pub fn new_from_enums(first: CodeFirst, second: CodeSecond, last_digit: u8) -> Self {
        Response(first as u16 + second as u16 + last_digit as u16)
    }

    pub fn service_ready() -> Response {
        Response::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 0)
    }

    pub fn success_transfering_file() -> Response {
        Response::new_from_enums(CodeFirst::Positive, CodeSecond::Syntax, 1)
    }
}
