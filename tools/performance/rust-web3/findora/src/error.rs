use std::fmt::Formatter;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    CheckTx,
    SyncTx,
    SendErr,
    Io(std::io::Error),
    Unknown,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CheckTx => write!(f, "tx check failed"),
            Error::SyncTx => write!(f, "tx not accepted by tendermint"),
            Error::SendErr => write!(f, "tx not sent"),
            Error::Io(e) => write!(f, "Io error {:?}", e),
            Error::Unknown => write!(f, "an unknown error happens"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}
