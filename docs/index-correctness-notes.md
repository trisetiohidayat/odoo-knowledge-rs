# Index Correctness Notes

## Fixture Assertions

The mini-addon fixture is covered by exact assertions in:

- parser unit tests for manifest, Python, XML, CSV, and JavaScript facts
- `crates/odoo-knowledge-core/tests/index_mini_addon.rs` for end-to-end module,
  dependency, model, field, method, XML, security CSV, and frontend symbol rows
- CLI/MCP smoke tests for representative tool payloads

## Python MVP Count Differences

Representative mini fixture parity initially matched for index stats, search,
`odoo_model_context`, and `odoo_method_chain`. Rust now intentionally extracts
JavaScript import symbols as `js_import`, so `frontend_symbols` can be higher
than the Python MVP heuristic parser on JavaScript-heavy modules.

Known sources of count differences to expect when comparing full Odoo checkouts:

- `frontend_symbols`: Rust Tree-sitter JavaScript parser extracts imports in
  addition to patch, registry, and class symbols.
- `module_dependencies`: duplicate addon manifest selection and missing source
  dependency diagnostics can differ when checkout layout includes embedded test
  addons or tooling copies.
- parser basis/confidence: Rust uses Tree-sitter basis labels and per-fact
  confidence metadata where available.

These differences should be reviewed by comparing representative payloads rather
than relying only on table counts. The static analysis remains an approximation
of source facts, not exact runtime Odoo behavior.
