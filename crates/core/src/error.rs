use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    Config(String),
    Time(jiff::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "database error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Json(e) => write!(f, "invalid JSON: {e}"),
            Error::Config(msg) => write!(f, "config error: {msg}"),
            Error::Time(e) => write!(f, "date/time error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<jiff::Error> for Error {
    fn from(e: jiff::Error) -> Self {
        Error::Time(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}
