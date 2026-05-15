# Production Rollout Results

## Endpoint

- Rust: `https://mcp-odoo-rs.trisetio.my.id/mcp`
- Python comparison: `https://mcp-odoo.trisetio.my.id/mcp`

## Backup

- Backup directory recorded in `/tmp/odoo-rs-rollout-backup-dir`.
- Backup includes previous binary, production config/env, raw SQLite files, and a consistent SQLite `.backup` copy.

## Materialized Contexts

For `odoo-19`:

- `odoo_module_context`: 647 payloads
- `odoo_xmlid_lookup`: 29,767 payloads
- Payload version: `mcp-context-v1`
- Live-vs-materialized validation: 60 checked, 0 mismatches

## Post-Rollout Validation

- Public MCP accuracy: 10/10 passed, 0 failed
- Ranking metrics: top1 accuracy 1.0, top5 recall 1.0, MRR 1.0, NDCG@5 1.0
- SQLite query plan audit: 11/11 passed, 0 failed
- Public health: `status=ok`, 13 tools

## Public Benchmark Summary

Concurrency 8 over HTTPS:

| Endpoint | Payload | OK | Failed | Mean ms | Median ms | P95 ms | Max ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| Rust | light tools/list + ping, 100 req | 100 | 0 | 70.78 | 57.52 | 255.12 | 354.76 |
| Python | light tools/list + ping, 100 req | 100 | 0 | 107.67 | 62.75 | 542.73 | 1432.48 |
| Rust | `odoo_search sale.order`, 80 req | 80 | 0 | 117.89 | 41.46 | 773.27 | 864.37 |
| Python | `odoo_search sale.order`, 80 req | 72 | 8 | 436.27 | 306.51 | 1267.59 | 1566.49 |

## Notes

- Public HTTPS benchmarks are noisy because they include nginx, TLS, network jitter, and server load.
- Rust endpoint had zero failures in the post-rollout benchmark set.
- Python comparison had failures on concurrent search.
