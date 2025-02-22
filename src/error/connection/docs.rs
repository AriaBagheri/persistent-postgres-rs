use super::*;

impl StandardErrorDocsTrait for PersistentPostgresConnectionError {
    fn docs(&self) -> &'static str {
        match self {
            PersistentPostgresConnectionError::NoAddress => {""}
            PersistentPostgresConnectionError::FailedToEstablishConnection => {""}
            PersistentPostgresConnectionError::FailedToEstablishListener => {""}
        }
    }
}