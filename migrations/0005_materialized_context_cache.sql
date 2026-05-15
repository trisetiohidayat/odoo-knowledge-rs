CREATE TABLE IF NOT EXISTS materialized_tool_contexts (
    codebase_id INTEGER NOT NULL,
    tool_name TEXT NOT NULL,
    cache_key TEXT NOT NULL,
    payload_version TEXT NOT NULL,
    source_indexed_at TEXT,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(codebase_id, tool_name, cache_key),
    FOREIGN KEY(codebase_id) REFERENCES codebases(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_materialized_tool_contexts_lookup
    ON materialized_tool_contexts(codebase_id, tool_name, cache_key, payload_version, source_indexed_at);
