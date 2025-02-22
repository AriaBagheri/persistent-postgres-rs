use super::*;

impl StandardErrorCausesTrait for PersistentPostgresListenerError {
    fn causes(&self) -> Option<&'static str> {
        match self {
            PersistentPostgresListenerError::FailedToAddChannelListener => {
                Some("Possible causes could be the listener already existing. Or else.")
            }
        }
    }
}