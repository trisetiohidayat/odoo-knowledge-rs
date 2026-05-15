# Security Review Checklist

This project is intended to be a local, read-only Odoo source knowledge server.
Use this checklist before exposing an index through HTTP or a remote MCP client.

## Source And Filesystem Safety

- [ ] Confirm configured codebase roots are trusted local checkouts.
- [ ] Confirm indexing only reads source files and writes only to the configured SQLite database.
- [ ] Confirm no CLI or MCP tool writes into an Odoo source checkout.
- [ ] Review tool responses for absolute path exposure before sharing outside a trusted environment.
- [ ] Prefer relative `file_path` provenance in tool payloads; treat `codebase.root_path` as potentially sensitive.

## Secrets And Logs

- [ ] Do not log bearer token values or secret environment variable contents.
- [ ] Do not configure bearer tokens in shell history or committed config files.
- [ ] Review error responses before exposing HTTP publicly; avoid returning host paths when not needed.

## HTTP/MCP Exposure

- [ ] For public HTTP deployments, restrict access with TLS, firewall allowlists, proxy limits, or optional bearer auth.
- [ ] Put the server behind a TLS-terminating reverse proxy for remote access.
- [ ] Restrict reverse-proxy request size and timeout values.
- [ ] Use network ACLs or firewall rules when the endpoint is for a small team.
- [ ] Keep `/health` responses free of secrets.

## Operations

- [ ] Back up SQLite indexes before migration or upgrade work.
- [ ] Rebuild indexes from source after parser/schema upgrades when compatibility is uncertain.
- [ ] Review dependencies with `cargo audit` or an equivalent process before release.
