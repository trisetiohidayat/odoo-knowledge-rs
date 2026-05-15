# Odoo Knowledge RS MCP: Complete Technical Documentation

> Status: production rollout completed for `https://mcp-odoo-rs.trisetio.my.id/mcp`.
>
> Scope: this document explains the Rust MCP server, its public endpoint, all MCP tools, indexing/search concepts, performance work, accuracy safeguards, materialized context cache, deployment, validation, and operational procedures.
>
> Important limitation: this project performs static source-code analysis. Static analysis can approximate Odoo structure, inheritance, and relationships, but it must not be claimed as exact runtime Odoo registry behavior.

---

## Document Index

### Part A: Orientation

1. [Executive Summary](#1-executive-summary)
2. [Who This Document Is For](#2-who-this-document-is-for)
3. [System Goals](#3-system-goals)
4. [Important URLs And Endpoints](#4-important-urls-and-endpoints)
5. [Glossary For Non-Specialists](#5-glossary-for-non-specialists)
6. [External References](#6-external-references)

### Part B: Architecture

7. [High-Level Architecture](#7-high-level-architecture)
8. [Repository Structure](#8-repository-structure)
9. [Runtime Components](#9-runtime-components)
10. [Request Lifecycle](#10-request-lifecycle)
11. [Database And Indexing Model](#11-database-and-indexing-model)
12. [SQLite Migrations](#12-sqlite-migrations)
13. [Transport Protocol](#13-transport-protocol)
14. [HTTP Routes](#14-http-routes)
15. [Authentication And Public Access](#15-authentication-and-public-access)

### Part C: MCP Tool Reference

16. [Tool System Overview](#16-tool-system-overview)
17. [`odoo_search`](#17-odoo_search)
18. [`odoo_impact_analysis`](#18-odoo_impact_analysis)
19. [`odoo_context_bundle`](#19-odoo_context_bundle)
20. [`odoo_trace_business_flow`](#20-odoo_trace_business_flow)
21. [`odoo_find_extension_point`](#21-odoo_find_extension_point)
22. [`odoo_debug_hypotheses`](#22-odoo_debug_hypotheses)
23. [`odoo_compare_symbol`](#23-odoo_compare_symbol)
24. [`odoo_module_context`](#24-odoo_module_context)
25. [`odoo_model_context`](#25-odoo_model_context)
26. [`odoo_method_chain`](#26-odoo_method_chain)
27. [`odoo_field_context`](#27-odoo_field_context)
28. [`odoo_view_chain`](#28-odoo_view_chain)
29. [`odoo_xmlid_lookup`](#29-odoo_xmlid_lookup)

### Part D: Performance And Accuracy Work Completed

30. [Tasklist Phases Completed](#30-tasklist-phases-completed)
31. [Phase 1: Measurement Baseline](#31-phase-1-measurement-baseline)
32. [Phase 2: Low-Risk Hot Path Optimization](#32-phase-2-low-risk-hot-path-optimization)
33. [Phase 3: In-Memory Response Cache](#33-phase-3-in-memory-response-cache)
34. [Phase 4: Exact-Match Fast Paths](#34-phase-4-exact-match-fast-paths)
35. [Phase 5: SQLite Index And Query Plan Audit](#35-phase-5-sqlite-index-and-query-plan-audit)
36. [Phase 6: Odoo-Aware Ranking](#36-phase-6-odoo-aware-ranking)
37. [Phase 7: Precomputed Context Materialization](#37-phase-7-precomputed-context-materialization)
38. [Phase 8: Production Rollout](#38-phase-8-production-rollout)

### Part E: Validation, Benchmarks, And Operations

39. [Accuracy Evaluation Suite](#39-accuracy-evaluation-suite)
40. [SQLite Query Plan Audit](#40-sqlite-query-plan-audit)
41. [Benchmarking Methodology](#41-benchmarking-methodology)
42. [Public Rust vs Python Result Summary](#42-public-rust-vs-python-result-summary)
43. [Deployment Procedure](#43-deployment-procedure)
44. [Rollback Procedure](#44-rollback-procedure)
45. [Materialized Cache Operations](#45-materialized-cache-operations)
46. [Monitoring And Health Checks](#46-monitoring-and-health-checks)
47. [Security And Exposure Notes](#47-security-and-exposure-notes)
48. [Known Tradeoffs And Risks](#48-known-tradeoffs-and-risks)
49. [Remaining Stretch Goals](#49-remaining-stretch-goals)
50. [Appendix: Key Files](#50-appendix-key-files)

---

## 1. Executive Summary

The Rust MCP server, called **Odoo Knowledge RS**, is a production HTTP JSON-RPC MCP server for querying indexed Odoo source code. It rebuilds the Python `odoo-knowledge` behavior in Rust while preserving MCP tool compatibility and JSON response shapes.

The production Rust endpoint is:

```text
https://mcp-odoo-rs.trisetio.my.id/mcp
```

The older Python endpoint is:

```text
https://mcp-odoo.trisetio.my.id/mcp
```

After the optimization and rollout work, the Rust endpoint has:

- Public HTTPS access through nginx.
- No bearer token requirement.
- 13 MCP tools.
- SQLite read connection pooling.
- Cached tool schemas.
- Lightweight `/health` output.
- In-memory response caching.
- Exact-match search fast paths.
- Odoo-aware ranking.
- Query-plan-audited SQLite indexes.
- Materialized context cache for selected tools.
- Accuracy regression checks.
- Public benchmark artifacts.

Post-rollout public validation passed:

- MCP accuracy: `10/10 passed`, `0 failed`.
- Ranking metrics: `top1_accuracy=1.0`, `top5_recall=1.0`, `MRR=1.0`, `NDCG@5=1.0`.
- SQLite query plan audit: `11/11 passed`, `0 failed`.
- Public health: `status=ok`, 13 tools.

---

## 2. Who This Document Is For

This document is for:

- Developers who maintain the Rust MCP server.
- Operators who deploy or restart the service.
- Users who call the MCP endpoint from agents such as Codex or other MCP clients.
- Reviewers who need to understand why Rust is faster and more reliable than the Python endpoint.
- Non-specialists who need definitions for protocol, database, caching, and ranking terminology.

---

## 3. System Goals

The system is designed to answer Odoo source-code questions quickly and reliably.

Primary goals:

1. Preserve Python-compatible MCP tool behavior.
2. Keep tool JSON shapes stable.
3. Use SQLite migrations as the compatibility boundary.
4. Improve latency without reducing accuracy.
5. Add measurable accuracy checks before claiming ranking improvements.
6. Avoid claiming exact runtime Odoo behavior from static analysis.

Non-goals:

- It does not execute Odoo.
- It does not replace Odoo registry runtime inspection.
- It does not modify Odoo source checkouts.
- It does not write into indexed Odoo repositories.

---

## 4. Important URLs And Endpoints

### Production Rust MCP Endpoint

```text
https://mcp-odoo-rs.trisetio.my.id/mcp
```

Used by MCP clients through JSON-RPC POST requests.

### Production Rust Health Endpoint

```text
https://mcp-odoo-rs.trisetio.my.id/health
```

Used for health checks and quick diagnostics.

### Production Rust Root Endpoint

```text
https://mcp-odoo-rs.trisetio.my.id/
```

Returns basic server metadata and tool names.

### Python Comparison Endpoint

```text
https://mcp-odoo.trisetio.my.id/mcp
```

Used as the baseline comparison service.

---

## 5. Glossary For Non-Specialists

### MCP

**MCP** means **Model Context Protocol**. It is a protocol that lets AI tools expose structured capabilities, called tools, to agents and applications. In this project, MCP is the protocol used by coding agents to ask Odoo source-code questions.

### JSON-RPC

**JSON-RPC** is a lightweight remote procedure call format using JSON. A client sends a JSON object with a `method`, optional `params`, and an `id`. The server replies with either a `result` or an `error`.

Example request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}
```

### Tool

A **tool** is a callable MCP capability. Example: `odoo_search` searches the indexed Odoo source code.

### Codebase

A **codebase** is one registered Odoo source checkout, such as `odoo-19` or `odoo-18`.

### Index

An **index** is a structured SQLite database created from scanning source files. It stores modules, models, fields, methods, XML records, views, menus, actions, symbols, and text chunks.

### Static Analysis

**Static analysis** means reading source files without running the program. This project parses Odoo files and builds structured facts from them. Static analysis is useful, but it cannot perfectly reproduce Odoo runtime behavior.

### SQLite

**SQLite** is an embedded database stored in a local file. This project stores its source-code index in SQLite.

### Migration

A **migration** is a versioned SQL change that creates or updates the database schema. In this project, migrations are the compatibility boundary.

### FTS5

**FTS5** is SQLite's full-text search engine. It allows fast lexical search over symbols and text chunks.

### WAL

**WAL** means **Write-Ahead Logging**. It is a SQLite mode that improves read/write concurrency by writing changes to a separate log before merging them into the main database.

### Cache

A **cache** stores computed results so repeated requests can be answered faster.

### In-Memory Cache

An **in-memory cache** lives inside the running process. It is very fast but disappears when the service restarts.

### Materialized Cache

A **materialized cache** stores precomputed responses in SQLite. It survives service restarts and can make the first request fast.

### Ranking

**Ranking** is the process of ordering search results so the most relevant result appears first.

### Top-1 Accuracy

**Top-1 accuracy** means the expected result appears as the first result.

### Top-5 Recall

**Top-5 recall** means the expected result appears anywhere in the first five results.

### MRR

**MRR** means **Mean Reciprocal Rank**. It rewards results that appear closer to rank 1.

### NDCG

**NDCG** means **Normalized Discounted Cumulative Gain**. It is a ranking-quality metric that rewards relevant results near the top.

### p95 Latency

**p95 latency** means 95% of requests finished at or below this duration. It is useful because average latency can hide slow tail behavior.

### Tail Latency

**Tail latency** means the slowest part of request distribution, such as p95, p99, and max latency.

### Reverse Proxy

A **reverse proxy** is a server, such as nginx, that receives public HTTPS traffic and forwards it to an internal service.

---

## 6. External References

The implementation and documentation use the following public references for theory and protocol background:

- MCP documentation: <https://modelcontextprotocol.io/>
- MCP specification: <https://spec.modelcontextprotocol.io/>
- JSON-RPC 2.0 specification: <https://www.jsonrpc.org/specification>
- SQLite FTS5 documentation: <https://www.sqlite.org/fts5.html>
- SQLite WAL documentation: <https://www.sqlite.org/wal.html>
- SQLite query planner overview: <https://www.sqlite.org/queryplanner.html>
- nginx proxy module documentation: <https://nginx.org/en/docs/http/ngx_http_proxy_module.html>
- axum Rust web framework documentation: <https://docs.rs/axum/>
- rusqlite documentation: <https://docs.rs/rusqlite/>

These links explain general concepts. The project-specific behavior is defined by this repository's code, migrations, tests, and deployment files.

---

## 7. High-Level Architecture

The Rust MCP system has four major layers:

```text
Client / Agent
  ↓ HTTPS JSON-RPC
nginx reverse proxy
  ↓ HTTP localhost proxy
Rust MCP server
  ↓ SQLite queries and caches
SQLite Odoo source index
```

### Layer 1: Client Or Agent

The client sends MCP JSON-RPC requests. Examples include:

- `initialize`
- `tools/list`
- `tools/call`
- `ping`

### Layer 2: nginx

nginx terminates HTTPS and forwards requests to the local Rust process on `127.0.0.1:8766`.

### Layer 3: Rust MCP Server

The Rust binary is installed as:

```text
<INSTALL_BIN>/odoo-knowledge-rs
```

The systemd service is:

```text
odoo-knowledge-rs.service
```

The Rust server handles routes, parses JSON-RPC, authorizes if configured, dispatches MCP tools, and returns JSON-RPC responses.

### Layer 4: SQLite Index

The SQLite database is:

```text
<ODOO_KNOWLEDGE_DB>
```

It stores parsed facts about Odoo codebases.

---

## 8. Repository Structure

Important repository locations:

| Path | Purpose |
|---|---|
| `crates/odoo-knowledge-cli/src/main.rs` | CLI, HTTP server, MCP JSON-RPC transport, tool dispatch |
| `crates/odoo-knowledge-core/src/services/mod.rs` | Tool implementation logic |
| `crates/odoo-knowledge-core/src/search/mod.rs` | Search, exact-match boosts, ranking |
| `crates/odoo-knowledge-core/src/storage/sqlite.rs` | Database opening and migration execution |
| `crates/odoo-knowledge-core/src/storage/schema.rs` | Migration registry |
| `migrations/` | SQLite schema migrations |
| `scripts/benchmark_mcp_concurrent.py` | Concurrent HTTP MCP benchmark |
| `scripts/evaluate_mcp_accuracy.py` | MCP accuracy regression runner |
| `scripts/audit_sqlite_query_plans.py` | SQLite query plan audit runner |
| `benchmarks/accuracy/mcp_accuracy_cases.json` | Accuracy fixture cases |
| `docs/mcp-performance-accuracy-tasklist.md` | Phase checklist |
| `docs/production-rollout-results.md` | Production rollout benchmark and validation results |

---

## 9. Runtime Components

### Rust Binary

The binary is built from the `odoo-knowledge-cli` package and installed as:

```bash
sudo install -m 0755 target/release/odoo-knowledge <INSTALL_BIN>/odoo-knowledge-rs
```

### systemd Service

The service definition is in:

```text
/etc/systemd/system/odoo-knowledge-rs.service
```

It runs:

```text
<INSTALL_BIN>/odoo-knowledge-rs serve
```

### Production Config

The production config is:

```text
<CONFIG_DIR>/production.toml
```

Current live configuration uses:

```toml
[server]
host = "127.0.0.1"
port = 8766
request_body_limit_bytes = 1048576
request_timeout_secs = 30
cors_allow_origin = "https://mcp-odoo-rs.trisetio.my.id"
```

The service itself listens only on localhost. Public access is provided through nginx.

---

## 10. Request Lifecycle

A typical MCP tool call follows this sequence:

```text
1. Client sends HTTPS POST to /mcp.
2. nginx forwards to 127.0.0.1:8766/mcp.
3. axum route receives JSON body.
4. Server checks optional bearer authorization.
5. Server reads JSON-RPC method.
6. For tools/call, server extracts tool name and arguments.
7. Server checks in-memory response cache.
8. If cache miss, server dispatches to the tool implementation.
9. Tool implementation may check materialized SQLite payload.
10. If materialized miss, tool executes live SQLite queries.
11. Server wraps payload into MCP content format.
12. Server writes JSON-RPC response.
13. Response may be cached for later calls.
```

The key performance improvements are located at steps 7 and 9.

---

## 11. Database And Indexing Model

The SQLite index contains tables for structured Odoo facts:

| Table | Meaning |
|---|---|
| `codebases` | Registered Odoo source trees |
| `modules` | Odoo addons/modules and manifest metadata |
| `module_dependencies` | Module dependency graph |
| `files` | Indexed source files |
| `symbols` | Generic symbols such as models, methods, fields, XMLIDs, views, modules |
| `models` | Odoo model class contributors |
| `fields` | Odoo field definitions |
| `methods` | Odoo method definitions |
| `xml_records` | XML record IDs |
| `views` | Odoo view definitions and inherit links |
| `actions` | Odoo action records |
| `menus` | Odoo menu records |
| `security_rules` | Access and security records |
| `frontend_symbols` | JavaScript/frontend symbols |
| `graph_edges` | Static relationship graph edges |
| `chunks` | Text chunks for contextual search |
| `index_diagnostics` | Indexing warnings and diagnostics |
| `fts_symbols` | FTS5 virtual table for symbol search |
| `fts_chunks` | FTS5 virtual table for text chunk search |
| `materialized_tool_contexts` | Precomputed tool payloads |

The index is produced by scanning and parsing Odoo source files. Python, XML, CSV, JavaScript, and manifest files contribute facts.

---

## 12. SQLite Migrations

Migrations are registered in `crates/odoo-knowledge-core/src/storage/schema.rs`.

### Current Migration Sequence

| Migration | Purpose |
|---|---|
| `0001_initial` | Base schema for codebases, modules, files, symbols, models, fields, methods, XML records, views, actions, menus, graph edges, chunks, diagnostics |
| `0002_fts` | FTS5 virtual tables for symbol and chunk search |
| `0003_lookup_indexes` | Lookup indexes for exact model/method/field/XMLID/module queries |
| `0004_hot_path_order_indexes` | Composite indexes for hot paths with ordering needs |
| `0005_materialized_context_cache` | Materialized tool context cache table and lookup index |

Migrations are idempotent. The migration runner records applied versions in `schema_migrations`.

### Why Migrations Matter

Migrations are the compatibility boundary. If the Rust and Python implementations need to interoperate or preserve behavior, the SQLite schema defines the stable contract.

---

## 13. Transport Protocol

The server uses HTTP JSON-RPC for MCP.

### Supported JSON-RPC Methods

| Method | Purpose |
|---|---|
| `initialize` | Returns MCP protocol version, capabilities, and server info |
| `tools/list` | Returns tool schemas |
| `tools/call` | Calls one MCP tool |
| `ping` | Lightweight connectivity check |

### MCP Tool Response Format

Tool results are wrapped like this:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{ ... pretty JSON payload ... }"
      }
    ],
    "isError": false
  }
}
```

The `text` field contains a JSON string. MCP clients parse this text as the tool payload.

---

## 14. HTTP Routes

### `/`

Returns basic metadata:

- Server name.
- Transport type.
- MCP endpoint path.
- Health path.
- Tool names.

### `/health`

Returns lightweight health information:

```json
{
  "status": "ok",
  "server": "odoo-knowledge-rs",
  "tools": 13,
  "indexed_codebases": 3,
  "cache": {
    "hits": 78,
    "misses": 12
  }
}
```

Earlier behavior exposed `database_path`; this was removed to reduce information exposure on the public endpoint.

### `/mcp`

Handles MCP JSON-RPC POST requests.

---

## 15. Authentication And Public Access

The production Rust endpoint was intentionally made public without bearer-token authentication. The service itself still listens on localhost; nginx exposes it publicly via HTTPS.

Security depends on:

- TLS at nginx.
- Firewall/network controls where needed.
- Request body limits.
- Timeout limits.
- Read-only MCP tool behavior.

Bearer authentication remains supported by configuration if `bearer_token_env` is set, but the live public configuration does not require it.

---

## 16. Tool System Overview

There are 13 MCP tools.

| Tool | Purpose |
|---|---|
| `odoo_search` | Hybrid lexical and metadata search |
| `odoo_impact_analysis` | Related symbols and graph edges for a target |
| `odoo_context_bundle` | Compact context bundle for a topic or symptom |
| `odoo_trace_business_flow` | Trace static business flow from model and method |
| `odoo_find_extension_point` | Find candidate extension points for a goal |
| `odoo_debug_hypotheses` | Generate static debugging hypotheses |
| `odoo_compare_symbol` | Compare a symbol across two codebases |
| `odoo_module_context` | Module manifest, dependencies, models, and views |
| `odoo_model_context` | Model contributors, fields, methods, views |
| `odoo_method_chain` | Static override chain for a model method |
| `odoo_field_context` | Field definitions and related view sample |
| `odoo_view_chain` | View chain by XMLID or model |
| `odoo_xmlid_lookup` | Exact XMLID lookup across records, views, actions, menus |

All tool input schemas are returned through `tools/list`.

---

<a id="17-odoo_search"></a>

## 17. `odoo_search`

### Purpose

`odoo_search` searches indexed Odoo facts and text chunks.

### Inputs

```json
{
  "query": "sale.order",
  "filters": {
    "codebase": "odoo-19",
    "module": "sale",
    "limit": 10
  }
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `query`
- `results.symbols`
- `results.chunks`
- `basis`
- `confidence`

### How It Works

The search system combines:

1. Exact symbol fast path.
2. SQLite FTS5 symbol search.
3. SQLite FTS5 chunk search.
4. Odoo-aware ranking adjustments.
5. Fallback from AND matching to OR matching if results are sparse.

### Exact-Match Fast Path

Exact matching checks `symbols.name` and `symbols.qualname` for known structured prefixes:

- `model:<name>`
- `method:<model.method>`
- `field:<model.field>`
- `xmlid:<module.name>`
- `view:<module.name>`
- `module:<module_name>`

This is why queries such as `sale.order.action_confirm` and `point_of_sale.product_template_form_view` now rank exact results first.

### Ranking

Search ranking uses SQLite FTS rank plus Odoo-specific adjustments. Exact structured matches get a strong boost. Query intent also boosts relevant kinds and paths.

Examples:

- Query mentions `model`: boost model symbols.
- Query mentions `field`: boost field symbols.
- Query mentions `view` or `xml`: boost XML/view results.
- Query mentions frontend terms: boost `static/src` paths.

### Accuracy Guarantees

The accuracy fixture checks exact model, method, field, XMLID, and module searches. Post-rollout result: all exact ranking checks pass at rank 1.

---

<a id="18-odoo_impact_analysis"></a>

## 18. `odoo_impact_analysis`

### Purpose

`odoo_impact_analysis` finds static relationships around a target symbol, file, or XMLID.

### Inputs

```json
{
  "target": "sale.order.action_confirm",
  "codebase": "odoo-19"
}
```

### Output Concepts

- `matches`: indexed symbols matching the target.
- `outgoing_edges`: graph edges from the target.
- `incoming_edges`: graph edges pointing to the target.
- `related_symbols`: symbols in the same files as matched symbols.

### How It Works

The tool normalizes the target by removing a `symbol:` prefix if present, queries `symbols`, then queries `graph_edges` for incoming and outgoing relationships.

### Important Limitation

This is static graph analysis. It does not prove exact runtime impact in an Odoo registry.

---

<a id="19-odoo_context_bundle"></a>

## 19. `odoo_context_bundle`

### Purpose

`odoo_context_bundle` builds a compact bundle for a query, topic, symbol, model, method, or debugging symptom.

### Inputs

```json
{
  "query": "available_in_pos product template",
  "codebase": "odoo-19",
  "module": "point_of_sale",
  "limit": 10
}
```

### Output Concepts

- Search results.
- Potential model context.
- Potential field/method/view context.
- Diagnostics or related facts depending on query.

### How It Works

The tool uses search and service-level helpers to gather a small amount of relevant context. It is intended for AI agents that need a compact starting point before deeper tool calls.

---

<a id="20-odoo_trace_business_flow"></a>

## 20. `odoo_trace_business_flow`

### Purpose

`odoo_trace_business_flow` traces a static business entrypoint from a model and method.

### Inputs

```json
{
  "model_name": "sale.order",
  "method_name": "action_confirm",
  "codebase": "odoo-19"
}
```

### Output Concepts

- Method chain.
- Related graph edges.
- Static call hints.
- Notes about static approximation.

### How It Works

The tool combines method-chain analysis and graph-edge lookup around the specified model method.

### Important Limitation

Odoo runtime method resolution can depend on installed modules, registry load order, monkey patches, and runtime context. This tool approximates using indexed source facts.

---

<a id="21-odoo_find_extension_point"></a>

## 21. `odoo_find_extension_point`

### Purpose

`odoo_find_extension_point` suggests places to extend Odoo behavior for a development goal.

### Inputs

```json
{
  "goal": "add validation before sale order confirmation",
  "codebase": "odoo-19",
  "module": "sale"
}
```

### Output Concepts

- Candidate methods.
- Candidate models.
- Candidate views or XML records.
- Search evidence.

### How It Works

The tool searches indexed facts, prioritizes likely extension points, and returns evidence rather than making unsupported runtime claims.

---

<a id="22-odoo_debug_hypotheses"></a>

## 22. `odoo_debug_hypotheses`

### Purpose

`odoo_debug_hypotheses` builds likely debugging hypotheses for a symptom.

### Inputs

```json
{
  "symptom": "sale order confirmation does not create delivery",
  "codebase": "odoo-19",
  "module": "sale"
}
```

### Output Concepts

- Hypotheses.
- Related symbols.
- Related files.
- Suggested checks.

### How It Works

The tool searches indexed context and creates static analysis hypotheses. It is intended to guide investigation, not to assert final causes.

---

<a id="23-odoo_compare_symbol"></a>

## 23. `odoo_compare_symbol`

### Purpose

`odoo_compare_symbol` compares a symbol across two indexed Odoo codebases.

### Inputs

```json
{
  "symbol": "sale.order.action_confirm",
  "left_codebase": "odoo-18",
  "right_codebase": "odoo-19"
}
```

### Output Concepts

- Matches in left codebase.
- Matches in right codebase.
- Summary status.

### How It Works

The tool queries `symbols` in both codebases by `name`, `qualname`, or `file_path`, then returns a static comparison summary.

---

<a id="24-odoo_module_context"></a>

## 24. `odoo_module_context`

### Purpose

`odoo_module_context` returns module-level context.

### Inputs

```json
{
  "module_name": "point_of_sale",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `module`
- `depends`
- `dependents`
- `models`
- `views`
- `profile`
- `basis`
- `confidence`

### How It Works

The tool first checks the materialized cache. If there is a valid payload for the current codebase `indexed_at` and payload version, it returns that payload. Otherwise it performs live SQL queries against `modules`, `module_dependencies`, `models`, and `views`.

### Materialized Cache

This tool is materialized for `odoo-19` production:

- 647 module context payloads generated.
- Live-vs-cache validation found 0 mismatches in sampled validation.

---

<a id="25-odoo_model_context"></a>

## 25. `odoo_model_context`

### Purpose

`odoo_model_context` returns context about an Odoo model.

### Inputs

```json
{
  "model_name": "sale.order",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `model`
- `contributors`
- `fields`
- `methods`
- `views`
- `profile`
- `basis`
- `confidence`

### How It Works

It queries:

- `models` for class contributors.
- `fields` for field definitions.
- `methods` for method definitions.
- `views` for related views.

### Why It Is Not Yet Materialized

It can produce large payloads for heavily extended models. It remains a candidate for future materialization, but Phase 7 started with safer tools to reduce stale/bloat risk.

---

<a id="26-odoo_method_chain"></a>

## 26. `odoo_method_chain`

### Purpose

`odoo_method_chain` returns a static override chain for a model method.

### Inputs

```json
{
  "model_name": "sale.order",
  "method_name": "action_confirm",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `model`
- `method`
- `chain`
- `profile`
- `note`
- `basis`
- `confidence`

### How It Works

The tool queries `methods` for matching model/method rows and sorts them using an approximate reverse dependency-based module load order.

### Important Note

The returned order approximates Odoo override order. Exact registry MRO may differ.

---

<a id="27-odoo_field_context"></a>

## 27. `odoo_field_context`

### Purpose

`odoo_field_context` returns field definitions and a sample of related views.

### Inputs

```json
{
  "model_name": "product.template",
  "field_name": "available_in_pos",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `model`
- `field`
- `definitions`
- `related_views_sample`
- `profile`
- `basis`
- `confidence`

### How It Works

It queries `fields` for exact model/field definitions and `views` for related model views.

---

<a id="28-odoo_view_chain"></a>

## 28. `odoo_view_chain`

### Purpose

`odoo_view_chain` returns view records by XMLID or by model.

### Inputs

By XMLID:

```json
{
  "xmlid_or_model": "point_of_sale.product_template_form_view",
  "codebase": "odoo-19"
}
```

By model:

```json
{
  "xmlid_or_model": "product.template",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `query`
- `views`
- `profile`
- `basis`
- `confidence`

### How It Works

If the input looks like an XMLID, the tool queries `views.xmlid` and `views.inherit_id`. Otherwise it queries by `views.view_model`.

---

<a id="29-odoo_xmlid_lookup"></a>

## 29. `odoo_xmlid_lookup`

### Purpose

`odoo_xmlid_lookup` performs exact XMLID lookup.

### Inputs

```json
{
  "xmlid": "point_of_sale.product_template_form_view",
  "codebase": "odoo-19"
}
```

### Output Shape

Top-level fields include:

- `codebase`
- `xmlid`
- `records`
- `views`
- `actions`
- `menus`
- `basis`
- `confidence`

### How It Works

The tool first checks the materialized cache. If cache is valid, it returns the stored payload. Otherwise it queries:

- `xml_records`
- `views`
- `actions`
- `menus`

### Materialized Cache

This tool is materialized for `odoo-19` production:

- 29,767 XMLID lookup payloads generated.
- Median materialized latency in local test was around sub-millisecond.

---

## 30. Tasklist Phases Completed

The project followed `docs/mcp-performance-accuracy-tasklist.md`.

Completed phases:

- Phase 1: Measurement Baseline.
- Phase 2: Low-Risk Hot Path Optimization.
- Phase 3: In-Memory Response Cache.
- Phase 4: Exact-Match Fast Paths.
- Phase 5: SQLite Index And Query Audit.
- Phase 6: Odoo-Aware Ranking.
- Phase 7: Precomputed Context Materialization.
- Phase 8: Production Rollout.

Remaining items are stretch goals only.

---

## 31. Phase 1: Measurement Baseline

### Work Completed

- Added reproducible benchmark scripts.
- Added public endpoint benchmark artifacts.
- Added accuracy fixture with real Odoo questions.
- Added metrics: top-1 accuracy, top-5 recall, MRR, NDCG.

### Key Files

- `scripts/benchmark_mcp_concurrent.py`
- `scripts/evaluate_mcp_accuracy.py`
- `benchmarks/accuracy/mcp_accuracy_cases.json`

### Why It Matters

Before optimizing, the system needs objective checks. Otherwise, a change could make the server faster while silently making answers worse.

---

## 32. Phase 2: Low-Risk Hot Path Optimization

### Work Completed

- Cached `tools/list` schemas at startup.
- Removed sensitive `database_path` from `/health`.
- Added SQLite read connection pool.
- Added per-request timing logs.
- Validated that requests do not serialize on one database mutex.

### Key Implementation Concepts

#### Tool Schema Cache

The tool schema list rarely changes during a process lifetime. Building it once avoids repeated JSON construction.

#### Connection Pool

Instead of a single `Mutex<Connection>`, the HTTP server uses multiple SQLite connections behind a small pool. Requests are distributed round-robin.

#### Lightweight Health

The health endpoint now exposes status, tool count, indexed codebase count, and cache counters, but not local filesystem paths.

---

## 33. Phase 3: In-Memory Response Cache

### Work Completed

- Added bounded process-local cache.
- Cache key uses tool name and normalized arguments.
- `odoo_search` uses short TTL.
- Context/exact lookup tools use longer process-lifetime style caching.
- Cache hit/miss counters exposed in `/health`.

### Why It Matters

Many MCP clients repeat identical tool calls. In-memory caching avoids repeated SQLite work and response assembly.

### Tradeoff

The cache disappears when the service restarts. Phase 7 adds persistent materialized cache for selected tools.

---

## 34. Phase 4: Exact-Match Fast Paths

### Work Completed

Exact structured search now supports:

- Model lookup.
- Method lookup.
- Field lookup.
- XMLID lookup.
- View lookup.
- Module lookup.

### Example Improvements

These now rank exact results first:

- `sale.order`
- `sale.order.action_confirm`
- `product.template.available_in_pos`
- `point_of_sale.product_template_form_view`
- `point_of_sale`

### Accuracy Result

Post-change accuracy fixture:

```text
10/10 passed
```

Ranking metrics:

```text
top1_accuracy = 1.0
top5_recall = 1.0
MRR = 1.0
NDCG@5 = 1.0
```

---

## 35. Phase 5: SQLite Index And Query Plan Audit

### Work Completed

- Added indexes for exact lookup hot paths.
- Added indexes for ordered model/field/method/view hot paths.
- Added query plan audit script.
- Verified FTS tables are used for lexical paths.
- Verified materialized cache lookup uses an index.

### Key Files

- `migrations/0003_lookup_indexes.sql`
- `migrations/0004_hot_path_order_indexes.sql`
- `scripts/audit_sqlite_query_plans.py`

### Why Query Plan Audit Matters

SQLite may choose a poor plan if indexes are missing or unsuitable. `EXPLAIN QUERY PLAN` shows whether a query uses the intended index or scans too much data.

### Final Result

```text
11/11 query plans passed
0 failed
```

---

## 36. Phase 6: Odoo-Aware Ranking

### Work Completed

Search ranking now considers Odoo-specific meaning:

- Exact structured matches.
- Symbol kind intent.
- Backend model paths.
- XML/view paths.
- Security paths.
- Frontend paths.
- Model contributor relevance.

### Examples

If a user asks about a model, model symbols and `models/` paths get boosted.

If a user asks about XML or views, XML/view results and `views/` paths get boosted.

If a user asks about frontend, `static/src` paths get boosted.

### Conservative Design

The ranking boost adjusts ordering but does not remove FTS results. Broad fallback remains available.

---

## 37. Phase 7: Precomputed Context Materialization

### Work Completed

Phase 7 created persistent materialized JSON cache for selected tools.

Materialized tools:

- `odoo_module_context`
- `odoo_xmlid_lookup`

### Schema

The table is `materialized_tool_contexts`.

Key fields:

- `codebase_id`
- `tool_name`
- `cache_key`
- `payload_version`
- `source_indexed_at`
- `payload_json`
- `created_at`

### Invalidation Strategy

A cached payload is valid only if:

1. The codebase ID matches.
2. The tool name matches.
3. The cache key matches.
4. The payload version matches.
5. The codebase indexed timestamp matches.

If any condition fails, the tool falls back to live query assembly.

### Populate Command

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  materialize-contexts --codebase odoo-19
```

### Validate Command

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  validate-materialized --codebase odoo-19 --limit 30
```

### Production Result

For `odoo-19`:

- `odoo_module_context`: 647 payloads.
- `odoo_xmlid_lookup`: 29,767 payloads.
- Validation: 60 checked, 0 mismatches.

---

## 38. Phase 8: Production Rollout

### Work Completed

- Captured public pre-rollout baseline.
- Backed up previous binary, config, env, raw SQLite files, WAL/SHM files, and a consistent SQLite `.backup` copy.
- Installed optimized binary.
- Restarted `odoo-knowledge-rs.service`.
- Materialized production `odoo-19` context cache.
- Ran public accuracy evaluation.
- Ran public HTTPS benchmarks.
- Compared Rust and Python endpoints.
- Documented rollout results in `docs/production-rollout-results.md`.

### Backup Location

```text
<BACKUP_DIR>
```

### Final Public Status

```text
https://mcp-odoo-rs.trisetio.my.id/health
```

Returns status `ok` and 13 tools.

---

## 39. Accuracy Evaluation Suite

### File

```text
scripts/evaluate_mcp_accuracy.py
```

### Fixture

```text
benchmarks/accuracy/mcp_accuracy_cases.json
```

### What It Tests

The suite tests real Odoo cases such as:

- `sale.order` exact model ranking.
- `sale.order.action_confirm` exact method ranking.
- `product.template.available_in_pos` exact field ranking.
- `point_of_sale.product_template_form_view` exact XMLID ranking.
- `point_of_sale` exact module ranking.
- Tool payload correctness for model, field, method, XMLID, and module context tools.

### Example Command

```bash
python3 scripts/evaluate_mcp_accuracy.py \
  --endpoint https://mcp-odoo-rs.trisetio.my.id/mcp \
  --output-json benchmarks/runs/public-mcp/rust-public-post-rollout-accuracy.json
```

### Metrics

- `passed`
- `failed`
- `top1_accuracy`
- `top5_recall`
- `mrr`
- `ndcg_at_5`

---

## 40. SQLite Query Plan Audit

### File

```text
scripts/audit_sqlite_query_plans.py
```

### What It Verifies

It checks that hot queries use expected indexes or FTS virtual table plans:

- Exact symbol name lookup.
- Exact symbol qualname lookup.
- Model context contributor lookup.
- Field definition lookup.
- Method chain lookup.
- XMLID lookup.
- View lookup.
- Module manifest lookup.
- Materialized context lookup.
- FTS symbol search.
- FTS chunk search.

### Example Command

```bash
python3 scripts/audit_sqlite_query_plans.py \
  --output-json benchmarks/runs/public-mcp/sqlite-query-plan-audit-post-rollout.json
```

### Final Result

```text
11 passed
0 failed
```

---

## 41. Benchmarking Methodology

### Light Benchmark

Uses `tools/list` and `ping` requests. This measures HTTP, JSON-RPC, schema response, nginx, TLS, and server overhead.

### Search Benchmark

Uses `tools/call` with `odoo_search sale.order` at concurrency 8.

### Why Public Benchmarks Are Noisy

Public HTTPS benchmarks include:

- DNS.
- TLS.
- nginx.
- Network jitter.
- Server load.
- OS scheduling.
- SQLite cache warmness.

Therefore, public benchmarks show real user experience, not pure language speed.

---

## 42. Public Rust vs Python Result Summary

Concurrency 8 over HTTPS:

| Endpoint | Payload | OK | Failed | Mean ms | Median ms | P95 ms | Max ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| Rust | light tools/list + ping, 100 req | 100 | 0 | 70.78 | 57.52 | 255.12 | 354.76 |
| Python | light tools/list + ping, 100 req | 100 | 0 | 107.67 | 62.75 | 542.73 | 1432.48 |
| Rust | `odoo_search sale.order`, 80 req | 80 | 0 | 117.89 | 41.46 | 773.27 | 864.37 |
| Python | `odoo_search sale.order`, 80 req | 72 | 8 | 436.27 | 306.51 | 1267.59 | 1566.49 |

### Interpretation

Rust is now better for production MCP usage because:

- It has lower mean latency.
- It has lower tail latency.
- It has zero failures in the measured search benchmark.
- It has exact-ranking accuracy safeguards.
- It has materialized context for selected tools.

---

## 43. Deployment Procedure

### Build

```bash
cargo build --release -p odoo-knowledge-cli
```

### Backup

Create a timestamped backup directory and copy:

- Previous binary.
- Production config.
- Environment file.
- SQLite database.
- WAL file if present.
- SHM file if present.
- SQLite `.backup` copy.

### Install Binary

```bash
sudo install -m 0755 target/release/odoo-knowledge <INSTALL_BIN>/odoo-knowledge-rs
```

### Restart Service

```bash
sudo systemctl restart odoo-knowledge-rs.service
```

### Check Service

```bash
systemctl is-active odoo-knowledge-rs.service
journalctl -u odoo-knowledge-rs.service -n 30 --no-pager
curl https://mcp-odoo-rs.trisetio.my.id/health
```

### Materialize Contexts

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  materialize-contexts --codebase odoo-19
```

### Validate

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  validate-materialized --codebase odoo-19 --limit 30
```

---

## 44. Rollback Procedure

If rollout fails, restore the previous binary and restart service.

### Restore Binary

```bash
sudo install -m 0755 \
  <BACKUP_DIR>/odoo-knowledge-rs \
  <INSTALL_BIN>/odoo-knowledge-rs
```

### Restart

```bash
sudo systemctl restart odoo-knowledge-rs.service
```

### Database Rollback

Database rollback should be used only if necessary. Migrations are additive, so the old binary can usually ignore new tables and indexes. If full DB rollback is required, stop the service and restore the backup database files together.

---

## 45. Materialized Cache Operations

### Populate

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  materialize-contexts --codebase odoo-19
```

### Validate

```bash
<INSTALL_BIN>/odoo-knowledge-rs \
  --config <CONFIG_DIR>/production.toml \
  validate-materialized --codebase odoo-19 --limit 30
```

### When To Regenerate

Regenerate materialized contexts after:

- Reindexing a codebase.
- Parser changes.
- Schema changes.
- Payload version changes.
- Major service logic changes for materialized tools.

### Why Regeneration Matters

Materialized payloads are precomputed. If source facts change and cache is not regenerated, stale answers could be served. The implementation prevents stale usage by checking `source_indexed_at` and payload version, but regeneration restores the fast path.

---

## 46. Monitoring And Health Checks

### Health Command

```bash
curl https://mcp-odoo-rs.trisetio.my.id/health
```

### Expected Fields

- `status`
- `server`
- `tools`
- `indexed_codebases`
- `cache.hits`
- `cache.misses`

### Service Command

```bash
systemctl status odoo-knowledge-rs.service
```

### Logs

```bash
journalctl -u odoo-knowledge-rs.service -n 100 --no-pager
```

The server logs request timing spans for MCP methods and tools.

---

## 47. Security And Exposure Notes

The public endpoint does not require bearer token authentication. This is intentional for the current deployment.

Security controls:

- Service binds to localhost.
- nginx exposes HTTPS.
- Request body limit is configured.
- Request timeout is configured.
- MCP tools are read-only with respect to Odoo source checkouts.
- `/health` avoids exposing the database path.

Operational caution:

- Do not expose secret environment values in logs.
- Do not return unnecessary host paths in public diagnostics.
- Keep backups protected because they contain indexed source metadata.

---

## 48. Known Tradeoffs And Risks

### Static Analysis Risk

Static facts may not match runtime behavior exactly.

### Cache Staleness Risk

Materialized payloads can become stale if index timestamps or payload versions are not checked. The implementation checks both and falls back to live query assembly when mismatched.

### Storage Growth Risk

Materialized JSON increases SQLite database size. This is acceptable for the current materialized scope but should be monitored before expanding to larger tools.

### Benchmark Noise

Public benchmark numbers can vary due to nginx, TLS, network jitter, and server load.

### Public Endpoint Risk

The endpoint is public. Although tools are read-only, public traffic can still consume CPU, memory, and I/O.

---

## 49. Remaining Stretch Goals

### Sustainable New-Version Onboarding

A one-hit onboarding workflow is available in `scripts/onboard_codebase.py`. It registers, indexes, validates, materializes, validates materialized payloads, optionally runs MCP accuracy checks, and optionally runs a benchmark smoke test for a new codebase such as `odoo-20`. See [New Odoo Version Onboarding](new-version-onboarding.md).

Parser evolution should follow the adaptive parser plan: detect unknown patterns, group them, generate fixtures, and promote only tested rules. See [Adaptive Parser Roadmap](adaptive-parser-roadmap.md).

Stretch goals from the tasklist remain optional:

- Persistent query-result cache keyed by index generation.
- Background cache warming for common tools and modules.
- Optional semantic reranking while keeping deterministic lexical search by default.
- Read-only SQLite immutable mode for deployed indexes.
- Load-test profiles for 1, 8, 32, and 64 concurrent clients.

These are not required for the completed production rollout.

---

## 50. Appendix: Key Files

### Core Runtime

- `crates/odoo-knowledge-cli/src/main.rs`
- `crates/odoo-knowledge-core/src/services/mod.rs`
- `crates/odoo-knowledge-core/src/search/mod.rs`
- `crates/odoo-knowledge-core/src/storage/sqlite.rs`
- `crates/odoo-knowledge-core/src/storage/schema.rs`

### Migrations

- `migrations/0001_initial.sql`
- `migrations/0002_fts.sql`
- `migrations/0003_lookup_indexes.sql`
- `migrations/0004_hot_path_order_indexes.sql`
- `migrations/0005_materialized_context_cache.sql`

### Scripts

- `scripts/benchmark_mcp_concurrent.py`
- `scripts/evaluate_mcp_accuracy.py`
- `scripts/audit_sqlite_query_plans.py`

### Documentation

- `docs/mcp-performance-accuracy-tasklist.md`
- `docs/production-rollout-results.md`
- `docs/mcp-rs-complete-technical-documentation.md`

### Benchmark Artifacts

- `benchmarks/runs/public-mcp/rust-public-post-rollout-accuracy.json`
- `benchmarks/runs/public-mcp/rust-public-post-rollout-light.json`
- `benchmarks/runs/public-mcp/rust-public-post-rollout-search-c8-r80.json`
- `benchmarks/runs/public-mcp/python-public-rollout-light.json`
- `benchmarks/runs/public-mcp/python-public-rollout-search-c8-r80.json`
- `benchmarks/runs/public-mcp/sqlite-query-plan-audit-post-rollout.json`

---

## Closing Summary

Odoo Knowledge RS is now the preferred MCP endpoint for production use. Compared with the Python endpoint, it has better concurrency stability, better exact-result ranking, lower average and tail latency in the measured public benchmark, and stronger validation/benchmark guardrails.

The recommended endpoint is:

```text
https://mcp-odoo-rs.trisetio.my.id/mcp
```

Use the Python endpoint primarily as a historical comparison baseline:

```text
https://mcp-odoo.trisetio.my.id/mcp
```
