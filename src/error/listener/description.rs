use super::*;

impl StandardErrorDescriptionTrait for PersistentPostgresListenerError {
    fn description(&self) -> Option<&'static str> {
        match self {
            PersistentPostgresListenerError::FailedToAddChannelListener => {
                Some("Failed to add channel to listener")
            }
        }
    }
}