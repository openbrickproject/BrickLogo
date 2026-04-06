use std::fmt;

#[derive(Debug, Clone)]
pub enum LogoError {
    Syntax { message: String, line: usize, col: usize },
    Runtime(String),
    Stop,
    Output(super::value::LogoValue),
}

impl fmt::Display for LogoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogoError::Syntax { message, line, col } => {
                write!(f, "{} (line {}, col {})", message, line, col)
            }
            LogoError::Runtime(msg) => write!(f, "{}", msg),
            LogoError::Stop => write!(f, "Stopped"),
            LogoError::Output(val) => write!(f, "Output: {}", val),
        }
    }
}

impl std::error::Error for LogoError {}

impl From<String> for LogoError {
    fn from(s: String) -> Self {
        LogoError::Runtime(s)
    }
}

pub type LogoResult<T> = Result<T, LogoError>;
