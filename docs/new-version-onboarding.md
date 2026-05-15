# New Odoo Version Onboarding

This document explains the sustainable workflow for adding a new Odoo source version, such as `odoo-20`, without reducing MCP performance or accuracy.

## One-Hit Command

Use `scripts/onboard_codebase.py` to run the safe onboarding sequence:

```bash
python3 scripts/onboard_codebase.py \
  --name odoo-20 \
  --path <ODOO_SOURCE_ROOT>/odoo-20 \
  --config <CONFIG_DIR>/production.toml \
  --binary odoo-knowledge-rs \
  --endpoint https://mcp-odoo-rs.trisetio.my.id/mcp
```

For maintaining an already indexed codebase that should follow the latest upstream commit, use [Latest Index Maintenance](latest-index-maintenance.md) instead of onboarding a duplicate codebase.

For local testing, use the release binary path:

```bash
python3 scripts/onboard_codebase.py \
  --name odoo-20 \
  --path <ODOO_SOURCE_ROOT>/odoo-20 \
  --config config/production.toml \
  --binary target/release/odoo-knowledge \
  --endpoint http://127.0.0.1:8766/mcp
```

## What The Script Does

The script runs these steps in order:

1. `add-codebase`
2. `index`
3. `validate`
4. `materialize-contexts`
5. `validate-materialized`
6. optional MCP accuracy evaluation
7. optional MCP benchmark smoke test
8. writes a JSON report

If a required step fails, the final report marks onboarding as failed and exits non-zero.

## Output Directory

Reports are written under:

```text
benchmarks/runs/onboarding/<timestamp>-<codebase>/
```

Each step gets its own JSON file plus a final `onboarding_report.json`.

## Performance Preservation

Performance is preserved by ensuring the new version receives the same optimizations as existing versions:

- SQLite migrations are applied.
- Lookup indexes are available.
- Exact-match fast paths work by `codebase_id`.
- Materialized context payloads are generated for supported tools.
- In-memory cache works after the service starts receiving requests.

## Accuracy Preservation

Accuracy is protected by:

- running `validate` after indexing,
- running `validate-materialized`,
- running MCP accuracy fixtures,
- keeping materialized payloads keyed by `codebase_id`, `payload_version`, and `source_indexed_at`.

## Parser Diagnostics

If the new Odoo version contains patterns the parser does not understand, `validate` should expose diagnostics. Repeated unknown patterns should be handled using the adaptive parser roadmap:

- [Adaptive Parser Roadmap](adaptive-parser-roadmap.md)

## Recommended Odoo 20 Checklist

- [ ] Source checkout exists locally.
- [ ] `cargo build --release -p odoo-knowledge-cli` succeeds.
- [ ] Run `scripts/onboard_codebase.py`.
- [ ] Review `validate` diagnostics.
- [ ] Review `validate-materialized` result.
- [ ] Run public or local accuracy eval.
- [ ] Run benchmark smoke.
- [ ] Add Odoo 20-specific accuracy fixtures if new behavior appears.
- [ ] Add parser fixtures before broadening parser behavior.

## Example MCP Query After Onboarding

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "odoo_search",
    "arguments": {
      "query": "sale.order",
      "filters": {
        "codebase": "odoo-20",
        "limit": 10
      }
    }
  }
}
```
