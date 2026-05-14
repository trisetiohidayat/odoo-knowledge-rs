# Production Readiness Checklist

This Rust implementation is currently an internal MVP. It is benchmarkable and
usable for local experimentation, but it should not be treated as production
ready until the checklist below is addressed.

## Parser Correctness

- [x] Replace the heuristic Python parser with Tree-sitter Python or
  `rustpython-parser`.
- [ ] Support multi-line Python field declarations.
- [ ] Support multi-line decorators and method signatures.
- [ ] Improve `_name`, `_inherit`, and `_inherits` extraction for complex but
  static expressions.
- [ ] Improve method line ranges.
- [ ] Improve method call extraction from real syntax nodes.
- [ ] Add parser confidence levels per extracted fact.
- [ ] Replace heuristic JavaScript parsing with Tree-sitter JavaScript.
- [ ] Extract POS/Owl imports, registry entries, patch targets, classes, and
  methods more accurately.

## Tool Coverage

- [ ] Implement `odoo_impact_analysis`.
- [ ] Implement `odoo_context_bundle`.
- [ ] Implement `odoo_trace_business_flow`.
- [ ] Implement `odoo_find_extension_point`.
- [ ] Implement `odoo_debug_hypotheses`.
- [ ] Implement `odoo_compare_symbol`.
- [ ] Add JSON schemas for all MCP tool inputs.
- [ ] Keep output shape compatible with the Python MVP where possible.

## Index Correctness

- [ ] Investigate table-count differences against Python MVP.
- [ ] Investigate `module_dependencies` count difference.
- [ ] Investigate `frontend_symbols` count difference.
- [ ] Compare representative tool payloads against Python output.
- [ ] Add exact fixture-based assertions for models, fields, methods, XML,
  security CSV, and JS symbols.
- [ ] Document where Rust intentionally differs from Python.

## Tests

- [ ] Add unit tests for manifest parsing.
- [ ] Add unit tests for Python parser fixtures.
- [ ] Add unit tests for XML parser fixtures.
- [ ] Add unit tests for CSV security parser.
- [ ] Add unit tests for JavaScript parser fixtures.
- [ ] Add integration tests for full mini-addon indexing.
- [ ] Add CLI smoke tests.
- [ ] Add MCP stdio smoke tests.
- [ ] Add HTTP JSON-RPC smoke tests.
- [ ] Add benchmark regression thresholds.

## Storage And Migrations

- [ ] Add a migration version table.
- [ ] Add an idempotent migration runner.
- [ ] Add schema compatibility tests.
- [ ] Add safe migration docs for existing SQLite indexes.
- [ ] Decide whether generated FTS tables should be rebuilt automatically.

## Indexer Operations

- [ ] Add incremental indexing by file hash or mtime.
- [ ] Add module-scoped reindexing.
- [ ] Add progress logging for long indexing runs.
- [ ] Add cancellation-safe transaction behavior tests.
- [ ] Add configurable parser concurrency.
- [ ] Add benchmark coverage for cold cache vs warm cache.

## HTTP And MCP Hardening

- [ ] Add graceful shutdown.
- [ ] Add request body size limits.
- [ ] Add request timeout handling.
- [ ] Add structured logging with `tracing`.
- [ ] Add CORS configuration.
- [ ] Add clear error mapping for JSON-RPC errors.
- [ ] Add bearer-token auth tests.
- [ ] Add health endpoint details for DB path and index metadata.
- [ ] Add optional read-only public mode guidance.

## Security

- [ ] Review path exposure in tool responses.
- [ ] Ensure no command path accepts arbitrary source writes.
- [ ] Ensure all source parsing is read-only.
- [ ] Avoid leaking secret environment values in logs or error messages.
- [ ] Document safe reverse-proxy deployment.

## Packaging And Deployment

- [ ] Add release build workflow.
- [ ] Add packaged binary artifact publishing.
- [ ] Finalize systemd unit.
- [ ] Finalize nginx/Caddy examples.
- [ ] Add install and upgrade docs.
- [ ] Add backup/restore docs for SQLite indexes.
- [ ] Add operational runbook.

## Benchmarking

- [ ] Add warmup iterations.
- [ ] Add p95 and p99 latency.
- [ ] Add memory usage measurement.
- [ ] Add CPU time measurement.
- [ ] Add concurrent MCP request benchmarks.
- [ ] Add benchmark comparison across Odoo 17, 18, and 19.
