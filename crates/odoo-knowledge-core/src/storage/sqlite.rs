use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::Result;
use crate::storage::schema::MIGRATIONS;

pub fn open_database(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut con = Connection::open(path)?;
    con.execute_batch("PRAGMA foreign_keys=ON;")?;
    run_migrations(&mut con)?;
    Ok(con)
}

pub fn run_migrations(con: &mut Connection) -> Result<()> {
    con.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )?;

    for (version, sql) in MIGRATIONS {
        let already_applied: i64 = con.query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version=?1",
            [version],
            |row| row.get(0),
        )?;
        if already_applied == 0 {
            con.execute_batch(sql)?;
            con.execute(
                "INSERT INTO schema_migrations(version) VALUES (?1)",
                params![version],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent_and_record_versions() {
        let mut con = Connection::open_in_memory().unwrap();
        run_migrations(&mut con).unwrap();
        run_migrations(&mut con).unwrap();

        let versions: i64 = con
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(versions, MIGRATIONS.len() as i64);
        assert_table_exists(&con, "codebases");
        assert_table_exists(&con, "symbols");
        assert_table_exists(&con, "fts_symbols");
        assert_table_exists(&con, "fts_chunks");
    }

    #[test]
    fn schema_contains_compatibility_tables() {
        let mut con = Connection::open_in_memory().unwrap();
        run_migrations(&mut con).unwrap();

        for table in [
            "codebases",
            "modules",
            "module_dependencies",
            "profiles",
            "profile_modules",
            "files",
            "symbols",
            "models",
            "fields",
            "methods",
            "xml_records",
            "views",
            "actions",
            "menus",
            "security_rules",
            "frontend_symbols",
            "graph_edges",
            "chunks",
            "index_diagnostics",
            "fts_symbols",
            "fts_chunks",
            "schema_migrations",
        ] {
            assert_table_exists(&con, table);
        }
    }

    fn assert_table_exists(con: &Connection, table: &str) {
        let count: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name=?1 AND type IN ('table', 'virtual table')",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }
}
