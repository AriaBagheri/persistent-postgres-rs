mod causes;
mod description;
mod docs;
mod solutions;
mod code;

use standard_error::traits::*;

pub enum PersistentPostgresConnectionError {
    NoAddress,
    FailedToEstablishConnection,
    FailedToEstablishListener,
}