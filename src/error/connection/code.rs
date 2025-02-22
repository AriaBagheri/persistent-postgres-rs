use super::*;

impl StandardErrorCodeTrait for PersistentPostgresConnectionError {
    fn code(&self) -> usize {
        match self {
            PersistentPostgresConnectionError::NoAddress => 5001,
            PersistentPostgresConnectionError::FailedToEstablishConnection => 5002,
            PersistentPostgresConnectionError::FailedToEstablishListener => 5003,
        }
    }
}