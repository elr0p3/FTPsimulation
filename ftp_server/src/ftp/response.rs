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

    pub fn closing_data_connection() -> Response {
        Response::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 6)
    }

    pub fn command_okay() -> Response {
        Response::new_from_enums(CodeFirst::Positive, CodeSecond::Syntax, 0)
    }

    pub fn bad_sequence_of_commands() -> Response {
        Response::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::Syntax,
            3,
        )
    }

    pub fn login_success() -> Response {
        Response::new_from_enums(
            CodeFirst::Positive,
            CodeSecond::AuthenticationAndAccounting,
            0,
        )
    }

    pub fn username_okay() -> Response {
        Response::new_from_enums(
            CodeFirst::PositiveIntermediate,
            CodeSecond::AuthenticationAndAccounting,
            1,
        )
    }

    pub fn file_unavailable() -> Response {
        Response::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::FileSystem,
            0,
        )
    }

    pub fn file_busy() -> Response {
        Response::new_from_enums(
            CodeFirst::TransientNegativeCompletion,
            CodeSecond::FileSystem,
            0,
        )
    }

    pub fn file_status_okay() -> Response {
        Response::new_from_enums(CodeFirst::PositivePreliminary, CodeSecond::FileSystem, 0)
    }

    pub fn cant_open_data_connection() -> Response {
        Response::new_from_enums(
            CodeFirst::TransientNegativeCompletion,
            CodeSecond::FileSystem,
            5,
        )
    }
}
