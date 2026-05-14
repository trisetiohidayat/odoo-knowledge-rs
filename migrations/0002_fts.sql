CREATE VIRTUAL TABLE IF NOT EXISTS fts_symbols USING fts5(
    codebase_id UNINDEXED,
    kind,
    name,
    qualname,
    module,
    file_path UNINDEXED
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_chunks USING fts5(
    codebase_id UNINDEXED,
    module,
    symbol_kind,
    symbol_name,
    text,
    file_path UNINDEXED
);

