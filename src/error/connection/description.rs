use super::*;

impl StandardErrorDescriptionTrait for PersistentPostgresConnectionError {
    fn description(&self) -> Option<&'static str> {
        match self {
            PersistentPostgresConnectionError::NoAddress => Some("No address found for postgres."),
            PersistentPostgresConnectionError::FailedToEstablishConnection => {
                Some("Failed to establish a connection to postgres.")
            }
            PersistentPostgresConnectionError::FailedToEstablishListener => {
                Some("Failed to establish a listener to postgres.")
            }
        }
    }
}