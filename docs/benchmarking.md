# Benchmarking

Use `scripts/benchmark_python_vs_rust.py` to compare the original Python MVP
with the Rust implementation.

Current Rust parser status:

- Python parser: Tree-sitter based.
- JavaScript parser: Tree-sitter.

The benchmark creates isolated SQLite databases under `benchmarks/runs/<run_id>`
so repeated runs do not modify the development or production indexes.

Measured areas:

- full index rebuild time
- SQLite table counts
- SQLite database size
- diagnostics count
- CLI search latency
- tool latency for model, field, method, view, XMLID, and module context
- output size and scenario-specific result metrics
- warmup iterations excluded from latency summaries
- p95 and p99 query/tool latency
- child-process CPU time and max RSS samples
- optional regression thresholds via `--thresholds-json`
- cold index rebuild and optional warm reindex coverage via `--warm-index-iterations`

Example:

```bash
cd <REPO_ROOT>
python3 scripts/benchmark_python_vs_rust.py \
  --odoo-root <ODOO_SOURCE_ROOT>/odoo-19 \
  --index-iterations 1 \
  --warm-index-iterations 1 \
  --query-iterations 5 \
  --warmup-iterations 1 \
  --keep-dbs
```

Threshold file example:

```json
{
  "implementations": {
    "rust-tree-sitter": {
      "max_mean_index_ms": 30000,
      "scenarios": {
        "search_payment_screen": {
          "max_mean_ms": 250,
          "max_p95_ms": 400,
          "max_p99_ms": 500,
          "max_cpu_mean_ms": 250,
          "max_rss_kb": 200000
        }
      }
    }
  }
}
```

Run only Rust:

```bash
python3 scripts/benchmark_python_vs_rust.py --only rust --query-iterations 10
```

Outputs:

- `benchmark.json`: full machine-readable result
- `benchmark.md`: readable summary tables
- `python-index.db` and `rust-index.db` when `--keep-dbs` is used

Latest Rust-only Tree-sitter smoke result for Odoo 19:

- Index time: `18.09s`
- DB size: `117.59 MB`
- Models: `3752`
- Fields: `15486`
- Methods: `41690`
- XML records: `30672`
- Views: `3695`
- Frontend symbols: `7256`
