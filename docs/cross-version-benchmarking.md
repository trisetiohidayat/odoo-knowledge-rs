# Cross-Version Benchmarking

Use the main benchmark script once per Odoo checkout and keep the reports under
`benchmarks/runs`:

```bash
python3 scripts/benchmark_python_vs_rust.py --odoo-root <ODOO_SOURCE_ROOT>/odoo-17 --codebase odoo-17 --output-json benchmarks/runs/odoo-17.json --output-md benchmarks/runs/odoo-17.md
python3 scripts/benchmark_python_vs_rust.py --odoo-root <ODOO_SOURCE_ROOT>/odoo-18 --codebase odoo-18 --output-json benchmarks/runs/odoo-18.json --output-md benchmarks/runs/odoo-18.md
python3 scripts/benchmark_python_vs_rust.py --odoo-root <ODOO_SOURCE_ROOT>/odoo-19 --codebase odoo-19 --output-json benchmarks/runs/odoo-19.json --output-md benchmarks/runs/odoo-19.md
```

For HTTP concurrency testing, start `odoo-knowledge serve` and run:

```bash
python3 scripts/benchmark_mcp_concurrent.py --endpoint http://127.0.0.1:8765/mcp --concurrency 16 --requests 200
```
