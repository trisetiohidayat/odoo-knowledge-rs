# SQLite Migration Operations

`odoo-knowledge-rs` treats SQLite migrations as the compatibility boundary.
The migration runner records applied versions in `schema_migrations` and applies
new migrations idempotently when `open_database` is called.

## Existing Index Upgrade

1. Stop any running `odoo-knowledge serve` process.
2. Back up the SQLite files: `index.db`, `index.db-wal`, and `index.db-shm` when present.
3. Start the new binary once against the database; migrations run automatically.
4. Run `odoo-knowledge validate --codebase <name>` after startup.
5. Reindex from source if parser or schema release notes say indexed facts changed.

## FTS Rebuild Policy

Generated FTS5 tables are treated as rebuildable derived data. Current migrations
create FTS tables if absent and normal full reindexing clears/repopulates FTS
rows for the selected codebase. If an FTS schema changes in a future migration,
prefer dropping/recreating the generated FTS table in that migration and then
performing a full reindex.

## Recovery

If migration fails, restore the backed up SQLite files and rerun with logs enabled
using `RUST_LOG=debug`. Because the source checkout is read-only input, indexes can
always be deleted and rebuilt from source.
