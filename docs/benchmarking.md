# Benchmarking

Use `scripts/benchmark_python_vs_rust.py` to compare the original Python MVP
with the Rust implementation.

Current Rust parser status:

- Python parser: Tree-sitter based.
- JavaScript parser: heuristic.

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

Example:

```bash
cd /home/ubuntu/odoo/odoo-knowledge-rs
python3 scripts/benchmark_python_vs_rust.py \
  --odoo-root /home/ubuntu/odoo/odoo-19 \
  --index-iterations 1 \
  --query-iterations 5 \
  --keep-dbs
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
