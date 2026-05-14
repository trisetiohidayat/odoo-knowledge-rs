use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
use crate::storage::schema::{FTS_SCHEMA, INITIAL_SCHEMA};

pub fn open_database(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let con = Connection::open(path)?;
    con.execute_batch("PRAGMA foreign_keys=ON;")?;
    con.execute_batch(INITIAL_SCHEMA)?;
    con.execute_batch(FTS_SCHEMA)?;
    Ok(con)
}
