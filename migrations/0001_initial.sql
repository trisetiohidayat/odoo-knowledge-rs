PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS codebases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    root_path TEXT NOT NULL,
    odoo_series TEXT,
    version TEXT,
    git_remote TEXT,
    git_branch TEXT,
    git_commit TEXT,
    indexed_at TEXT
);

CREATE TABLE IF NOT EXISTS modules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    manifest_path TEXT NOT NULL,
    installable INTEGER,
    auto_install INTEGER,
    application INTEGER,
    summary TEXT,
    UNIQUE(codebase_id, name),
    FOREIGN KEY(codebase_id) REFERENCES codebases(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS module_dependencies (
    codebase_id INTEGER NOT NULL,
    module TEXT NOT NULL,
    depends_on TEXT NOT NULL,
    PRIMARY KEY(codebase_id, module, depends_on)
);

CREATE TABLE IF NOT EXISTS profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    UNIQUE(codebase_id, name)
);

CREATE TABLE IF NOT EXISTS profile_modules (
    profile_id INTEGER NOT NULL,
    module TEXT NOT NULL,
    PRIMARY KEY(profile_id, module)
);

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    path TEXT NOT NULL,
    rel_path TEXT NOT NULL,
    language TEXT,
    role TEXT,
    sha1 TEXT,
    UNIQUE(codebase_id, rel_path)
);

CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualname TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER,
    basis TEXT NOT NULL DEFAULT 'unknown',
    confidence TEXT NOT NULL DEFAULT 'medium'
);

CREATE TABLE IF NOT EXISTS models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    model_name TEXT NOT NULL,
    class_name TEXT,
    inherit TEXT,
    inherits TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS fields (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    model_name TEXT,
    field_name TEXT NOT NULL,
    field_type TEXT,
    comodel TEXT,
    compute TEXT,
    inverse TEXT,
    search TEXT,
    related TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS methods (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    model_name TEXT,
    class_name TEXT,
    method_name TEXT NOT NULL,
    decorators TEXT,
    calls_super INTEGER NOT NULL DEFAULT 0,
    calls TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS xml_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    xmlid TEXT NOT NULL,
    record_model TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER,
    UNIQUE(codebase_id, xmlid, file_path)
);

CREATE TABLE IF NOT EXISTS views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    xmlid TEXT,
    view_model TEXT,
    inherit_id TEXT,
    priority TEXT,
    xpath_count INTEGER NOT NULL DEFAULT 0,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    xmlid TEXT,
    action_model TEXT,
    res_model TEXT,
    view_id TEXT,
    file_path TEXT,
    line_start INTEGER
);

CREATE TABLE IF NOT EXISTS menus (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    xmlid TEXT,
    action_ref TEXT,
    parent_ref TEXT,
    file_path TEXT,
    line_start INTEGER
);

CREATE TABLE IF NOT EXISTS security_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    kind TEXT NOT NULL,
    name TEXT,
    model_ref TEXT,
    group_ref TEXT,
    permissions TEXT,
    file_path TEXT,
    line_start INTEGER
);

CREATE TABLE IF NOT EXISTS frontend_symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    target TEXT,
    category TEXT,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS graph_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    source_kind TEXT NOT NULL,
    source TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target TEXT NOT NULL,
    file_path TEXT,
    line_start INTEGER,
    confidence TEXT NOT NULL DEFAULT 'medium',
    basis TEXT NOT NULL DEFAULT 'graph_resolver'
);

CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    module TEXT,
    symbol_kind TEXT,
    symbol_name TEXT,
    text TEXT NOT NULL,
    file_path TEXT,
    line_start INTEGER,
    line_end INTEGER
);

CREATE TABLE IF NOT EXISTS index_diagnostics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codebase_id INTEGER NOT NULL,
    severity TEXT NOT NULL,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    file_path TEXT,
    line_start INTEGER
);

