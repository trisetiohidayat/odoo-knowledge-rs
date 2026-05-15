# AGENTS.md

This Rust project rebuilds the Python `odoo-knowledge` project while preserving
its architecture contract and MCP tool behavior.

Authoritative architecture spec:

- `<PYTHON_ODOO_KNOWLEDGE_ROOT>/AGENTS.md`

Local Rust-specific rules:

- Keep `odoo-knowledge-core` free of CLI and HTTP concerns.
- Keep all tool JSON shapes compatible with the Python implementation.
- Treat SQLite schema migrations as the compatibility boundary.
- Add parser fixtures before broadening parser behavior.
- Do not claim static analysis as exact runtime Odoo behavior.
- Do not commit server-specific absolute paths, credentials, bearer tokens,
  private keys, or live backup paths.
- Use placeholders such as `<CONFIG_DIR>`, `<DATA_DIR>`, `<INSTALL_BIN>`,
  `<ODOO_SOURCE_ROOT>`, and `<ODOO_KNOWLEDGE_DB>` in public documentation.

## Project Summary

Odoo Knowledge RS is a Rust MCP server and CLI for indexing Odoo source code and
serving read-only source knowledge to coding agents through HTTP JSON-RPC MCP.
It indexes Odoo modules, models, fields, methods, XML records, views, actions,
menus, security rules, frontend symbols, graph edges, and searchable text chunks
into SQLite.

The current production Rust MCP endpoint is:

- `https://mcp-odoo-rs.trisetio.my.id/mcp`

The older Python comparison endpoint is:

- `https://mcp-odoo.trisetio.my.id/mcp`

Live deployment currently uses a shared SQLite index for Python and Rust. Treat
Rust as the preferred writer for future indexing/reindexing unless explicitly
asked otherwise.

## Indexed Odoo Versions

The live shared index currently contains:

- `odoo-17`
- `odoo-18`
- `odoo-19`

When adding a new version such as `odoo-20`, prefer the one-hit onboarding
workflow in `scripts/onboard_codebase.py` and document the result.

When refreshing an existing version to a newer upstream commit, prefer the
guarded dry-run-first workflow in `scripts/update_indexed_codebase.py`. Do not
run `--apply` against a live/shared index unless explicitly requested.

## Main MCP Tools

The server exposes 13 MCP tools:

- `odoo_search`
- `odoo_impact_analysis`
- `odoo_context_bundle`
- `odoo_trace_business_flow`
- `odoo_find_extension_point`
- `odoo_debug_hypotheses`
- `odoo_compare_symbol`
- `odoo_module_context`
- `odoo_model_context`
- `odoo_method_chain`
- `odoo_field_context`
- `odoo_view_chain`
- `odoo_xmlid_lookup`

Preserve tool names, input schemas, top-level response fields, and MCP response
wrapping unless a user explicitly requests a breaking change and compatibility
is documented.

The server also exposes MCP prompts via `prompts/list` and `prompts/get` for
codebase-selection and investigation guidance. Keep prompt responses plain,
stable, and aligned with the indexed Odoo codebase naming rules.

## Performance And Accuracy Work Already Completed

The Rust MCP server has completed phases 1-8 in
`docs/mcp-performance-accuracy-tasklist.md`:

1. Measurement baseline.
2. Low-risk HTTP/MCP hot path optimization.
3. In-memory response cache.
4. Exact-match fast paths.
5. SQLite index and query plan audit.
6. Odoo-aware ranking.
7. Precomputed materialized context cache.
8. Production rollout and public benchmark comparison.

Materialized context currently covers:

- `odoo_module_context`
- `odoo_xmlid_lookup`

Materialized payload validity is keyed by codebase, tool, cache key, payload
version, and source `indexed_at` timestamp. If those do not match, code must
fall back to live query assembly.

## Important Documentation

Start with:

- `docs/INDEX.md`
- `docs/mcp-rs-complete-technical-documentation.md`
- `docs/mcp-performance-accuracy-tasklist.md`
- `docs/production-rollout-results.md`
- `docs/new-version-onboarding.md`
- `docs/latest-index-maintenance.md`
- `docs/adaptive-parser-roadmap.md`

## Validation Commands

Before claiming MCP behavior is safe after code changes, run the most relevant
subset of:

```bash
python3 -m py_compile scripts/evaluate_mcp_accuracy.py scripts/audit_sqlite_query_plans.py scripts/benchmark_mcp_concurrent.py scripts/onboard_codebase.py
cargo test -p odoo-knowledge-cli mcp -- --nocapture
cargo test -p odoo-knowledge-core migrations -- --nocapture
cargo build --release -p odoo-knowledge-cli
```

For a running MCP endpoint, run:

```bash
python3 scripts/evaluate_mcp_accuracy.py --endpoint <MCP_ENDPOINT>/mcp
python3 scripts/audit_sqlite_query_plans.py --db <ODOO_KNOWLEDGE_DB>
```

For materialized cache validation:

```bash
odoo-knowledge-rs --config <CONFIG_DIR>/production.toml validate-materialized --codebase <CODEBASE> --limit 30
```

## Adding New Odoo Versions

Use:

```bash
python3 scripts/onboard_codebase.py \
  --name odoo-20 \
  --path <ODOO_SOURCE_ROOT>/odoo-20 \
  --config <CONFIG_DIR>/production.toml \
  --binary odoo-knowledge-rs \
  --endpoint https://mcp-odoo-rs.trisetio.my.id/mcp
```

Review diagnostics after indexing. If parser diagnostics reveal repeated unknown
patterns, follow `docs/adaptive-parser-roadmap.md`: generate fixtures first,
then broaden parser behavior, then rerun accuracy and migration tests.

## Security And Git Hygiene

Before pushing, scan staged files for:

- server absolute paths,
- credentials,
- private keys,
- bearer tokens,
- IP addresses,
- large DB/index artifacts.

Do not commit files from live system directories. Do not commit benchmark SQLite
DBs. `benchmarks/runs/` is intentionally ignored for generated run artifacts.
