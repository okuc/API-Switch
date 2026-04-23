pub mod dao;
mod schema;

use crate::error::AppError;
use rusqlite::Connection;
use std::sync::Mutex;

/// Macro to safely lock the database connection
macro_rules! lock_conn {
    ($mutex:expr) => {
        $mutex
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?
    };
}

pub(crate) use lock_conn;

pub use dao::*;

/// Database connection wrapper
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    /// Open database at default location
    pub fn open() -> Result<Self, AppError> {
        let db_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("api-switch");

        std::fs::create_dir_all(&db_dir)
            .map_err(|e| AppError::Database(format!("Failed to create db dir: {e}")))?;

        let db_path = db_dir.join("api-switch.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| AppError::Database(format!("Failed to open db: {e}")))?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Database(format!("Failed to set pragmas: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create all tables
    pub fn create_tables(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        schema::create_tables(&conn)
    }
}
