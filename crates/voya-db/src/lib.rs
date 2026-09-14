//! SQLite and JSON persistence boundary for VoyaVPN.
//!
//! This crate owns the fresh schema, repository mapping, and the only place
//! where typed domain blobs become SQLite `TEXT`.

mod blob;
mod database;
mod error;
mod executor;
mod repos;
mod reset;

pub use blob::BlobError;
pub use database::{
    manual_database_reset_command, Database, DatabaseSession, UnitOfWork, DATABASE_NAME,
};
pub use error::{DbError, Result};
pub use repos::*;
pub use reset::{move_database_aside, DatabaseBackup};
