# Install And Upgrade

## Build

```bash
cargo build --release -p odoo-knowledge-cli
sudo install -m 0755 target/release/odoo-knowledge <INSTALL_BIN>/odoo-knowledge
```

## Configure

```bash
sudo mkdir -p <CONFIG_DIR> <DATA_DIR>
sudo cp config/production.toml <CONFIG_DIR>/production.toml
sudo cp examples/odoo-knowledge-rs.service /etc/systemd/system/odoo-knowledge.service
```

Edit `<CONFIG_DIR>/production.toml`, set the database path and CORS
origin. The default production config listens on `0.0.0.0:8765` without bearer
auth, so restrict access with your firewall or reverse proxy if needed.

## Start

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now odoo-knowledge.service
sudo systemctl status odoo-knowledge.service
```

## Upgrade

1. Stop the service.
2. Back up SQLite database files, including `-wal` and `-shm`.
3. Install the new binary.
4. Start the service so migrations run.
5. Run `odoo-knowledge validate --codebase <name>`.
6. Reindex codebases when parser or schema release notes require it.
