# Operations Runbook

## Install Or Upgrade

1. Build a release binary with `cargo build --release -p odoo-knowledge-cli`.
2. Install `target/release/odoo-knowledge` to `<INSTALL_BIN>/odoo-knowledge`.
3. Install `config/production.toml` to `<CONFIG_DIR>/production.toml` and adjust paths.
4. Start via the systemd unit in `examples/odoo-knowledge-rs.service`.

## Backup And Restore

- Stop the service before backup when possible.
- Back up the configured SQLite database plus `-wal` and `-shm` files.
- Restore all three files together, then start the service and run `validate`.
- If restore is not available, rebuild indexes from registered source checkouts.

## Reverse Proxy Guidance

- Terminate TLS at nginx/Caddy.
- Forward only `/mcp`, `/health`, and optionally `/`.
- Enforce request body limits and request timeouts at the proxy.
- `config/production.toml` publishes HTTP on `0.0.0.0:8765` without bearer auth by default.
- If exposing the service broadly, keep TLS, firewall allowlists, request body limits, and timeouts at the proxy/network layer.
- Do not publish logs containing secret environment values.
