# Latest Index Maintenance

This document explains how to keep an already indexed Odoo codebase current when the upstream Odoo Git repository receives new commits.

The workflow is intentionally guarded so it can be used without disturbing the existing SQLite index by default.

## Index

- [Purpose](#purpose)
- [Safety Model](#safety-model)
- [Export Current Index Inventory](#export-current-index-inventory)
- [Dry-Run Latest Update](#dry-run-latest-update)
- [Apply Latest Update](#apply-latest-update)
- [What The Update Script Runs](#what-the-update-script-runs)
- [Reports](#reports)
- [Rollback Notes](#rollback-notes)
- [Operational Checklist](#operational-checklist)
- [Glossary](#glossary)

## Purpose

Odoo source branches continue moving after this MCP index is created. The SQLite index records the Git commit that was indexed at that time; it does not automatically follow upstream Git commits.

Maintenance therefore has two separate tasks:

1. Export the current index inventory so operators can see which codebase, branch, commit, and object counts are present.
2. Refresh one selected codebase only after a reviewed dry-run plan and explicit `--apply`.

## Safety Model

- `scripts/export_index_inventory.py` is read-only and opens SQLite only to count existing rows.
- `scripts/update_indexed_codebase.py` is dry-run by default and does not fetch, pull, reindex, materialize, restart, or mutate SQLite unless `--apply` is provided.
- Source paths and config paths are redacted in reports by default.
- The update script requires `--source-path`; it does not read `root_path` from the database so reports do not accidentally leak server paths.
- Generated reports are written under `benchmarks/runs/`, which is ignored and should not be committed.

## Export Current Index Inventory

Use this command to export the currently indexed versions and counts:

```bash
python3 scripts/export_index_inventory.py \
  --db <ODOO_KNOWLEDGE_DB> \
  --output-json benchmarks/runs/inventory/index-inventory.json \
  --output-md benchmarks/runs/inventory/index-inventory.md
```

By default the database path and codebase root paths are redacted as `<REDACTED>`.

Only use `--include-root-paths` for private operator reports that will not be committed:

```bash
python3 scripts/export_index_inventory.py \
  --db <ODOO_KNOWLEDGE_DB> \
  --include-root-paths
```

The inventory includes:

- database page statistics,
- codebase name, Odoo series, version, Git remote, branch, commit, and indexed timestamp,
- counts for modules, symbols, models, fields, methods, XML records, and views,
- materialized payload counts by MCP tool.

## Dry-Run Latest Update

Always run dry-run first. This command inspects local Git state and prints the exact planned steps without changing Git or SQLite:

```bash
python3 scripts/update_indexed_codebase.py \
  --codebase odoo-19 \
  --source-path <ODOO_SOURCE_ROOT>/odoo-19 \
  --config <CONFIG_DIR>/production.toml \
  --binary odoo-knowledge-rs \
  --endpoint https://mcp-odoo-rs.trisetio.my.id/mcp \
  --inventory-db <ODOO_KNOWLEDGE_DB>
```

Dry-run output includes:

- current local branch,
- current local `HEAD`,
- configured upstream reference if available,
- local upstream reference commit if already fetched,
- dirty working-tree status,
- planned commands and which steps mutate state.

Dry-run intentionally does not run `git fetch` because `git fetch` mutates `.git/FETCH_HEAD` and local remote-tracking refs.

## Apply Latest Update

Use `--apply` only after dry-run is reviewed and an index backup or snapshot policy exists:

```bash
python3 scripts/update_indexed_codebase.py \
  --apply \
  --codebase odoo-19 \
  --source-path <ODOO_SOURCE_ROOT>/odoo-19 \
  --config <CONFIG_DIR>/production.toml \
  --binary odoo-knowledge-rs \
  --endpoint https://mcp-odoo-rs.trisetio.my.id/mcp \
  --inventory-db <ODOO_KNOWLEDGE_DB>
```

The apply mode fails if the source checkout has local changes. Use `--allow-dirty` only for controlled private workflows where local changes are intentional.

Optional flags:

- `--skip-materialize` skips materialized payload regeneration and validation.
- `--skip-accuracy` skips MCP accuracy evaluation.
- `--skip-benchmark` skips light MCP benchmark.
- `--restart-command '<SERVICE_RESTART_COMMAND>'` runs a restart after all previous steps succeed.

## What The Update Script Runs

When `--apply` is used, the script runs steps sequentially and stops after the first required failure:

1. `git fetch --prune origin`
2. `git pull --ff-only`
3. `odoo-knowledge-rs --config <CONFIG_DIR>/production.toml index --codebase <CODEBASE>`
4. `odoo-knowledge-rs --config <CONFIG_DIR>/production.toml validate --codebase <CODEBASE>`
5. `odoo-knowledge-rs --config <CONFIG_DIR>/production.toml materialize-contexts --codebase <CODEBASE>`
6. `odoo-knowledge-rs --config <CONFIG_DIR>/production.toml validate-materialized --codebase <CODEBASE> --limit 30`
7. optional MCP accuracy evaluation
8. optional light MCP benchmark
9. optional inventory export
10. optional service restart

This protects accuracy by validating the live index and validating materialized payloads after reindexing.

## Reports

Reports are written under:

```text
benchmarks/runs/updates/<timestamp>-<codebase>/
```

Each step receives a JSON file with command, status, elapsed time, stdout, and stderr. The final summary is `update_report.json`.

Path redaction is enabled by default. For private local troubleshooting only, set:

```bash
ODOO_KNOWLEDGE_REPORT_PATHS=include python3 scripts/update_indexed_codebase.py ...
```

Do not commit reports that include private paths.

## Rollback Notes

The safest rollback is an infrastructure snapshot or SQLite backup taken before `--apply`.

If Git updated but indexing failed, the source checkout can usually be returned to the previous `HEAD` recorded in `update_report.json`, then the same codebase can be reindexed again. This is an operator action and should be done carefully because the SQLite index may already have partial refreshed rows depending on where the failure occurred.

For production, prefer these practices:

- backup the SQLite database before `--apply`,
- run update during a maintenance window,
- keep generated reports outside Git,
- restart the MCP service only after validation passes.

## Operational Checklist

- [ ] Export current inventory.
- [ ] Confirm the source checkout is the expected Odoo branch.
- [ ] Run dry-run update.
- [ ] Review dirty status and planned commands.
- [ ] Confirm index backup or snapshot exists.
- [ ] Run `--apply`.
- [ ] Review `validate` output.
- [ ] Review `validate-materialized` output.
- [ ] Review accuracy and benchmark reports when an endpoint is provided.
- [ ] Export post-update inventory.

## Glossary

- **Index**: SQLite data derived from Odoo source files so MCP tools can answer questions quickly.
- **Codebase**: A named Odoo source checkout such as `odoo-19`.
- **Git commit**: A unique identifier for an exact source version.
- **HEAD**: The current Git commit checked out in a source directory.
- **Upstream**: The remote branch a local Git branch tracks.
- **Dry-run**: A safe planning mode that prints what would happen without changing files or the database.
- **Materialized payload**: Precomputed MCP response content stored in SQLite for hot-path tools.
- **Validation**: Consistency checks run after indexing to detect parser/indexing issues.
