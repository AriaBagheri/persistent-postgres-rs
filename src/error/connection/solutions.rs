use super::*;

impl StandardErrorSolutionsTrait for PersistentPostgresConnectionError {
    fn solutions(&self) -> Option<&'static str> {
        Some(match self {
            PersistentPostgresConnectionError::NoAddress => {""}
            PersistentPostgresConnectionError::FailedToEstablishConnection => {""}
            PersistentPostgresConnectionError::FailedToEstablishListener => {""}
        })
    }
}