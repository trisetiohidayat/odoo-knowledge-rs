# MCP Performance And Accuracy Tasklist

Goal: make the Rust MCP server substantially faster and more relevant while preserving Python-compatible MCP tool behavior and JSON shapes.

## Ground Rules

- [ ] Preserve existing MCP tool names, request shapes, and response shapes.
- [ ] Keep CLI/HTTP concerns out of `odoo-knowledge-core`.
- [ ] Treat SQLite migrations as the compatibility boundary.
- [ ] Do not claim static analysis as exact runtime Odoo behavior.
- [ ] Measure latency and relevance before claiming improvements.

## Phase 1: Measurement Baseline

- [x] Add a reproducible local benchmark for HTTP MCP `tools/list`, `ping`, and representative `tools/call` payloads.
- [x] Add public endpoint benchmark documentation for `mcp-odoo.trisetio.my.id` vs `mcp-odoo-rs.trisetio.my.id`.
- [x] Record p50, p95, p99, max latency, throughput, failure count, response bytes, and server version.
- [x] Add an accuracy evaluation fixture with real Odoo questions and expected top results.
- [x] Track top-1 accuracy, top-5 recall, MRR, and NDCG for search-like tools.

## Phase 2: Low-Risk Hot Path Optimizations

- [x] Cache `tools/list` schemas at server startup instead of rebuilding them per request.
- [x] Keep `/health` lightweight and avoid exposing local filesystem paths.
- [x] Use a read connection pool for concurrent SQLite-backed MCP tool calls.
- [x] Add per-route timing spans for MCP method, tool name, success/failure, and elapsed time.
- [x] Validate that concurrent requests do not serialize on a single database mutex.

## Phase 3: In-Memory Response Cache

- [x] Add an optional bounded LRU cache for read-only MCP tool responses.
- [x] Key cache entries by tool name and normalized arguments; keep process-lifetime cache tied to the opened index.
- [x] Cache `tools/list` indefinitely for process lifetime.
- [x] Cache exact lookup/context tools for process lifetime.
- [x] Cache broad `odoo_search` results with a short TTL.
- [x] Expose cache hit/miss counters in logs or lightweight diagnostics.

## Phase 4: Exact-Match Fast Paths

- [x] Add fast path for exact model lookup before broad search.
- [x] Add fast path for exact method lookup scoped by model/codebase.
- [x] Add fast path for exact field lookup scoped by model/codebase.
- [x] Add fast path for exact XMLID lookup by `module.name`.
- [x] Add fast path for module manifest/context lookup by module name.
- [x] Fall back to existing broad search behavior when exact lookup does not match.

## Phase 5: SQLite Index And Query Audit

- [x] Audit all SQL used by MCP tools with `EXPLAIN QUERY PLAN`.
- [x] Add missing indexes for frequent lookup predicates.
- [x] Add composite indexes for `codebase_id + model/module/name` query patterns.
- [x] Confirm FTS tables are used for lexical search paths.
- [x] Add migration tests proving new indexes are created idempotently.
- [x] Benchmark cold and warm query performance after each migration.

## Phase 6: Odoo-Aware Ranking

- [x] Define ranking weights for exact symbol, model, method, field, XMLID, and module matches.
- [x] Boost Odoo-semantic paths such as `models/`, `views/`, `security/`, and `static/src` based on query intent.
- [x] Boost inherited model contributors when the query mentions an Odoo model.
- [x] Penalize broad text-only matches when exact structured matches exist.
- [x] Evaluate ranking changes against the accuracy fixture before merging.

## Phase 7: Precomputed Context Materialization

- [x] Identify high-cost tools suitable for materialized JSON context.
- [x] Add a compatibility-safe table for precomputed tool/context payloads.
- [x] Populate precomputed context during post-index finalization.
- [x] Invalidate precomputed context when index timestamp or payload/schema version changes.
- [x] Serve materialized payloads before falling back to live query assembly.

## Phase 8: Production Rollout

- [x] Deploy optimized binary to staging endpoint.
- [x] Run public HTTPS benchmark before and after deployment.
- [x] Run accuracy evaluation before and after deployment.
- [x] Compare Python and Rust endpoints with identical payload sets.
- [x] Document observed improvements and known regressions.
- [x] Roll out to `mcp-odoo-rs.trisetio.my.id` only after zero MCP shape regressions.

## Stretch Goals

- [ ] Add a persistent query-result cache keyed by index generation.
- [ ] Add background cache warming for common tools and modules.
- [ ] Add optional semantic reranking while keeping lexical/metadata search deterministic by default.
- [ ] Add read-only SQLite immutable mode for deployed indexes when no writes are expected.
- [ ] Add load-test profiles for 1, 8, 32, and 64 concurrent clients.
