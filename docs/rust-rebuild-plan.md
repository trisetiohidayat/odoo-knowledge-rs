# Rust Rebuild Plan

## Phase 1: Compatible Foundation

- Workspace crates: core, CLI, server.
- Config loader with TOML + env overrides.
- SQLite schema compatible with Python MVP.
- Codebase registry commands.
- Manifest scanner and module table population.

## Phase 2: Indexer Parity

- Python parser with Tree-sitter or `rustpython-parser`.
- XML parser for records, views, actions, menus.
- CSV access parser.
- JavaScript parser with Tree-sitter.
- Symbol, chunk, and graph edge generation.

## Phase 3: Query Tool Parity

- FTS search.
- Module/model/field/method/view/XMLID tools.
- Impact analysis and context bundle.
- Business flow trace approximation.

## Phase 4: Production MCP

- stdio JSON-RPC transport.
- HTTP JSON-RPC transport.
- Bearer auth.
- systemd and reverse proxy examples.
- Benchmarks and Python-vs-Rust comparison scripts.

