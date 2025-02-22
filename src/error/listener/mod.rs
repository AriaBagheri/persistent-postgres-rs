mod causes;
mod code;
mod description;
mod docs;
mod solutions;

use standard_error::traits::*;
pub enum PersistentPostgresListenerError {
    FailedToAddChannelListener,
}