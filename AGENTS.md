# AGENTS.md

This Rust project rebuilds `/home/ubuntu/odoo/odoo-knowledge` while preserving
its architecture contract and MCP tool behavior.

Authoritative architecture spec:

- `/home/ubuntu/odoo/odoo-knowledge/AGENTS.md`

Local Rust-specific rules:

- Keep `odoo-knowledge-core` free of CLI and HTTP concerns.
- Keep all tool JSON shapes compatible with the Python implementation.
- Treat SQLite schema migrations as the compatibility boundary.
- Add parser fixtures before broadening parser behavior.
- Do not claim static analysis as exact runtime Odoo behavior.

