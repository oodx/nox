use std::fmt;

#[derive(Debug)]
pub enum Error {
    Hyper(hyper::Error),
    Http(http::Error),
    Io(std::io::Error),
    #[cfg(feature = "config")]
    Yaml(serde_yaml::Error),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Hyper(e) => write!(f, "Hyper error: {}", e),
            Error::Http(e) => write!(f, "HTTP error: {}", e),
            Error::Io(e) => write!(f, "IO error: {}", e),
            #[cfg(feature = "config")]
            Error::Yaml(e) => write!(f, "YAML error: {}", e),
            Error::Other(s) => write!(f, "Error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Error::Http(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(feature = "config")]
impl From<serde_yaml::Error> for Error {
    fn from(e: serde_yaml::Error) -> Self {
        Error::Yaml(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;