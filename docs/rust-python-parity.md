# Rust vs Python MVP Parity Notes

This document records representative compatibility checks between the Rust
rebuild and the Python MVP behavior oracle. The goal is shape compatibility for
agent-facing tools, while preserving the Rust implementation notes from
`AGENTS.md`: static analysis must not be claimed as exact runtime Odoo behavior.

## Fixture Used

Representative checks use the Rust repository fixture at
`tests/fixtures/addons/mini_sale` registered as codebase `fixtures` in both
implementations.

Commands used:

```bash
python3 -m odoo_knowledge.cli --db /tmp/py.db add-codebase \
  --name fixtures --path <REPO_ROOT>/tests/fixtures
python3 -m odoo_knowledge.cli --db /tmp/py.db index --codebase fixtures
python3 -m odoo_knowledge.cli --db /tmp/py.db search sale.order --codebase fixtures --limit 5
python3 -m odoo_knowledge.cli --db /tmp/py.db tool odoo_model_context \
  '{"model_name":"sale.order","codebase":"fixtures"}'
python3 -m odoo_knowledge.cli --db /tmp/py.db tool odoo_method_chain \
  '{"model_name":"sale.order","method_name":"action_confirm","codebase":"fixtures"}'

cargo run -p odoo-knowledge-cli -- --db /tmp/rs.db add-codebase \
  --name fixtures --path <REPO_ROOT>/tests/fixtures
cargo run -p odoo-knowledge-cli -- --db /tmp/rs.db index --codebase fixtures
cargo run -p odoo-knowledge-cli -- --db /tmp/rs.db search sale.order --codebase fixtures --limit 5
cargo run -p odoo-knowledge-cli -- --db /tmp/rs.db tool odoo_model_context \
  '{"model_name":"sale.order","codebase":"fixtures"}'
cargo run -p odoo-knowledge-cli -- --db /tmp/rs.db tool odoo_method_chain \
  '{"model_name":"sale.order","method_name":"action_confirm","codebase":"fixtures"}'
```

## Representative Results

| Area | Python MVP | Rust rebuild | Status |
| --- | --- | --- | --- |
| Index stats | 1 module, 5 files, 1 model, 1 field, 2 methods, 1 XML record, 1 view, 3 frontend symbols at time of comparison | Same counts at time of comparison | Compatible baseline; Rust now also extracts JS imports |
| Search top-level shape | `codebase`, `query`, `results`, `basis`, `confidence` | Same keys | Compatible |
| Search results shape | `results.symbols[]` and `results.chunks[]` with kind/name/module/file/rank fields | Same shape | Compatible |
| `odoo_model_context` | `codebase`, `model`, `contributors`, `fields`, `methods`, `views`, `profile`, `basis`, `confidence` | Same top-level shape | Compatible |
| `odoo_method_chain` | `codebase`, `model`, `method`, `chain`, `profile`, `note`, `basis`, `confidence` | Same top-level shape | Compatible |

## Intentional Differences

- Rust parser provenance uses newer basis labels such as
  `tree_sitter_python_parse` and `tree_sitter_javascript_parse`; Python MVP may
  use older heuristic labels in existing indexes.
- Rust Tree-sitter JavaScript now extracts import symbols as `js_import`, so
  frontend symbol counts can be higher than the Python MVP heuristic parser.
- Rust has stricter MCP `inputSchema` definitions with
  `additionalProperties: false`; Python MVP advertises placeholder schemas in
  stdio MCP.
- Rust newly implemented tool coverage may use argument names that are clearer
  for the Rust API, for example `model_name` + `method_name` for
  `odoo_trace_business_flow` and `left_codebase` + `right_codebase` for
  `odoo_compare_symbol`. Python MVP accepts `entrypoint` and
  `from_codebase`/`to_codebase` respectively.
- Rust static tools include explicit notes that outputs are indexed static
  approximations, not exact runtime Odoo behavior.
- JSON object key order is not treated as meaningful.

## Current Follow-Up

- Broaden parity checks from the mini fixture to a real Odoo checkout.
- Compare representative payloads for the newly added Rust tools once Python
  MVP fixture coverage is expanded for those exact inputs.
- Keep SQLite migrations as the compatibility boundary for indexed facts.
