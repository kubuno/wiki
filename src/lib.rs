/// The database namespace this module owns: a PostgreSQL schema, a MySQL
/// database, or the SQLite file ATTACHed under that name. Never write outside
/// it.
pub const SCHEMA: &str = "wiki";

pub mod config;
pub mod errors;
pub mod events;
/// FilesClient + name helpers: CLIENT face of the `drive` module (delegated storage).
pub use kubuno_drive::client as files_client;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod router;
pub mod services;
pub mod state;
pub mod sync;
