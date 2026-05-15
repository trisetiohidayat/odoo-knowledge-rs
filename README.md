# Odoo Knowledge RS

Rust rebuild of `odoo-knowledge`: a local, read-only Odoo codebase index and
MCP server for coding agents.

The Python implementation remains the behavior oracle while this version is
being built.

Current parser status:

- Python: Tree-sitter based parser for models, fields, methods, decorators,
  `super()` detection, and call names.
- XML: record, view, action, and menu parser.
- CSV: `ir.model.access.csv` parser.
- JavaScript: Tree-sitter based parser for POS/Owl patch, registry, and class symbols.

## Documentation

- [Documentation Index](docs/INDEX.md)
- [Complete MCP Technical Documentation](docs/mcp-rs-complete-technical-documentation.md)
- [Production Rollout Results](docs/production-rollout-results.md)

## Development

```bash
cargo run -p odoo-knowledge-cli -- --config config/development.toml list-codebases
```

## Benchmarking

Compare the Python MVP with the Rust implementation:

```bash
python3 scripts/benchmark_python_vs_rust.py \
  --odoo-root <ODOO_SOURCE_ROOT>/odoo-19 \
  --index-iterations 1 \
  --query-iterations 5
```

See [docs/benchmarking.md](docs/benchmarking.md).

## Production Shape

- Binary: `<INSTALL_BIN>/odoo-knowledge`
- Config: `<CONFIG_DIR>/production.toml`
- Data: `<DATA_DIR>/index.db`
- Public access through nginx/Caddy reverse proxy.
