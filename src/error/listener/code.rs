use super::*;

impl StandardErrorCodeTrait for PersistentPostgresListenerError {
    fn code(&self) -> usize {
        match self {
            PersistentPostgresListenerError::FailedToAddChannelListener => 5010,
        }
    }
}