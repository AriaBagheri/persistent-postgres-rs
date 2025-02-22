pub mod connection;
pub mod listener;

use crate::error::connection::PersistentPostgresConnectionError;

pub enum PersistentPostgresError {
    ConnectionError(PersistentPostgresConnectionError)
}