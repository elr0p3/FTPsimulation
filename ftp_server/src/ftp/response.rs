#![allow(dead_code)]

#[derive(Clone, Copy, Debug)]
pub struct ResponseCode(pub u16);

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

impl ResponseCode {
    pub fn new_from_enums(first: CodeFirst, second: CodeSecond, last_digit: u8) -> Self {
        ResponseCode(first as u16 + second as u16 + last_digit as u16)
    }

    pub fn service_ready() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 0)
    }

    pub fn success_transfering_file() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Syntax, 1)
    }

    pub fn success_uploading_file() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 6)
    }

    pub fn closing_data_connection() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 6)
    }

    pub fn file_action_not_taken() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::FileSystem,
            3,
        )
    }

    pub fn command_okay() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Syntax, 0)
    }

    pub fn bad_sequence_of_commands() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::Syntax,
            3,
        )
    }

    pub fn login_success() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::Positive,
            CodeSecond::AuthenticationAndAccounting,
            0,
        )
    }

    pub fn unauthorized() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::AuthenticationAndAccounting,
            0,
        )
    }

    pub fn username_okay() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PositiveIntermediate,
            CodeSecond::AuthenticationAndAccounting,
            1,
        )
    }

    pub fn passive_ok() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 7)
    }

    pub fn all_ports_taken() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::Unspecified,
            1,
        )
    }

    pub fn closing_control_connection_success() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::Connections, 1)
    }

    pub fn file_unavailable() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::PermanentNegativeCompletion,
            CodeSecond::FileSystem,
            0,
        )
    }

    pub fn file_busy() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::TransientNegativeCompletion,
            CodeSecond::FileSystem,
            0,
        )
    }

    pub fn file_status_okay() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::PositivePreliminary, CodeSecond::FileSystem, 0)
    }

    pub fn file_action_okay() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::FileSystem, 0)
    }

    pub fn file_action_pending() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::PositiveIntermediate, CodeSecond::FileSystem, 0)
    }

    pub fn directory_action_okay() -> ResponseCode {
        ResponseCode::new_from_enums(CodeFirst::Positive, CodeSecond::FileSystem, 7)
    }

    pub fn cant_open_data_connection() -> ResponseCode {
        ResponseCode::new_from_enums(
            CodeFirst::TransientNegativeCompletion,
            CodeSecond::FileSystem,
            5,
        )
    }
}
