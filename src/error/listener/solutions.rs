use standard_error::traits::StandardErrorSolutionsTrait;
use crate::error::listener::PersistentPostgresListenerError;

impl StandardErrorSolutionsTrait for PersistentPostgresListenerError {
    fn solutions(&self) -> Option<&'static str> {
        match self {
            PersistentPostgresListenerError::FailedToAddChannelListener => {None}
        }
    }
}