use standard_error::traits::StandardErrorDocsTrait;
use crate::error::listener::PersistentPostgresListenerError;

impl StandardErrorDocsTrait for PersistentPostgresListenerError {
    fn docs(&self) -> &'static str {
        match self {
            PersistentPostgresListenerError::FailedToAddChannelListener => {""}
        }
    }
}