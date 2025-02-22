use super::*;

impl StandardErrorCausesTrait for PersistentPostgresConnectionError {
    fn causes(&self) -> Option<&'static str> {
        match self {
            PersistentPostgresConnectionError::NoAddress => {Some("Usually the cause of this error is bad configuration")}
            PersistentPostgresConnectionError::FailedToEstablishConnection => {Some("Cause of this problem could be a faulty internet connection. or bad connection string")},
            PersistentPostgresConnectionError::FailedToEstablishListener => None //Not sure about the cause of this error yet!
        }
    }
}