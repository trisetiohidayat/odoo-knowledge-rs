# Production Readiness Checklist

This Rust implementation is currently an internal MVP. It is benchmarkable and
usable for local experimentation, but it should not be treated as production
ready until the checklist below is addressed.

## Parser Correctness

- [x] Replace the heuristic Python parser with Tree-sitter Python or
  `rustpython-parser`.
- [x] Support multi-line Python field declarations.
- [x] Support multi-line decorators and method signatures.
- [x] Improve `_name`, `_inherit`, and `_inherits` extraction for complex but
  static expressions.
- [x] Improve method line ranges.
- [x] Improve method call extraction from real syntax nodes.
- [x] Add parser confidence levels per extracted fact.
- [x] Replace heuristic JavaScript parsing with Tree-sitter JavaScript.
- [x] Extract POS/Owl imports, registry entries, patch targets, classes, and
  methods more accurately.

## Tool Coverage

- [x] Implement `odoo_impact_analysis`.
- [x] Implement `odoo_context_bundle`.
- [x] Implement `odoo_trace_business_flow`.
- [x] Implement `odoo_find_extension_point`.
- [x] Implement `odoo_debug_hypotheses`.
- [x] Implement `odoo_compare_symbol`.
- [x] Add JSON schemas for all MCP tool inputs.
- [x] Keep output shape compatible with the Python MVP where possible.

## Index Correctness

- [x] Investigate table-count differences against Python MVP.
- [x] Investigate `module_dependencies` count difference.
- [x] Investigate `frontend_symbols` count difference.
- [x] Compare representative tool payloads against Python output.
- [x] Add exact fixture-based assertions for models, fields, methods, XML,
  security CSV, and JS symbols.
- [x] Document where Rust intentionally differs from Python.

## Tests

- [x] Add unit tests for manifest parsing.
- [x] Add unit tests for Python parser fixtures.
- [x] Add unit tests for XML parser fixtures.
- [x] Add unit tests for CSV security parser.
- [x] Add unit tests for JavaScript parser fixtures.
- [x] Add integration tests for full mini-addon indexing.
- [x] Add CLI smoke tests.
- [x] Add MCP stdio smoke tests.
- [x] Add HTTP JSON-RPC smoke tests.
- [x] Add benchmark regression thresholds.

## Storage And Migrations

- [x] Add a migration version table.
- [x] Add an idempotent migration runner.
- [x] Add schema compatibility tests.
- [x] Add safe migration docs for existing SQLite indexes.
- [x] Decide whether generated FTS tables should be rebuilt automatically.

## Indexer Operations

- [ ] Add incremental indexing by file hash or mtime.
- [ ] Add module-scoped reindexing.
- [x] Add progress logging for long indexing runs.
- [ ] Add cancellation-safe transaction behavior tests.
- [ ] Add configurable parser concurrency.
- [x] Add benchmark coverage for cold cache vs warm cache.

## HTTP And MCP Hardening

- [x] Add graceful shutdown.
- [x] Add request body size limits.
- [x] Add request timeout handling.
- [x] Add structured logging with `tracing`.
- [x] Add CORS configuration.
- [x] Add clear error mapping for JSON-RPC errors.
- [x] Add bearer-token auth tests.
- [x] Add health endpoint details for DB path and index metadata.
- [x] Add optional read-only public mode guidance.

## Security

- [x] Review path exposure in tool responses.
- [x] Ensure no command path accepts arbitrary source writes.
- [x] Ensure all source parsing is read-only.
- [x] Avoid leaking secret environment values in logs or error messages.
- [x] Document safe reverse-proxy deployment.
- [x] Add security review checklist.

## Packaging And Deployment

- [x] Add release build workflow.
- [x] Add packaged binary artifact publishing.
- [x] Finalize systemd unit.
- [x] Finalize nginx/Caddy examples.
- [x] Add install and upgrade docs.
- [x] Add backup/restore docs for SQLite indexes.
- [x] Add operational runbook.

## Benchmarking

- [x] Add warmup iterations.
- [x] Add p95 and p99 latency.
- [x] Add memory usage measurement.
- [x] Add CPU time measurement.
- [x] Add concurrent MCP request benchmarks.
- [x] Add benchmark comparison across Odoo 17, 18, and 19.
