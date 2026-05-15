#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path
from typing import Any

COUNT_TABLES = [
    "modules",
    "symbols",
    "models",
    "fields",
    "methods",
    "xml_records",
    "views",
    "materialized_tool_contexts",
]


def main() -> int:
    parser = argparse.ArgumentParser(description="Export indexed codebase inventory from SQLite")
    parser.add_argument("--db", required=True, type=Path, help="SQLite index database path")
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--output-md", type=Path)
    parser.add_argument(
        "--include-root-paths",
        action="store_true",
        help="Include source root paths. Default redacts them for public documentation.",
    )
    args = parser.parse_args()

    con = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    inventory = build_inventory(con, args.db, args.include_root_paths)
    text = json.dumps(inventory, indent=2, sort_keys=True)
    print(text)
    if args.output_json:
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(text + "\n", encoding="utf-8")
    if args.output_md:
        args.output_md.parent.mkdir(parents=True, exist_ok=True)
        args.output_md.write_text(render_markdown(inventory), encoding="utf-8")
    return 0


def build_inventory(con: sqlite3.Connection, db_path: Path, include_root_paths: bool) -> dict[str, Any]:
    codebases = []
    for row in con.execute(
        """
        SELECT id, name, root_path, odoo_series, version, git_remote, git_branch, git_commit, indexed_at
        FROM codebases
        ORDER BY name
        """
    ):
        codebase = dict(row)
        codebase_id = codebase["id"]
        if not include_root_paths:
            codebase["root_path"] = "<REDACTED>"
        codebase["counts"] = table_counts(con, codebase_id)
        codebase["materialized"] = materialized_counts(con, codebase_id)
        codebases.append(codebase)

    return {
        "database": {
            "path": str(db_path) if include_root_paths else "<REDACTED>",
            "page_count": pragma_scalar(con, "page_count"),
            "page_size": pragma_scalar(con, "page_size"),
            "freelist_count": pragma_scalar(con, "freelist_count"),
        },
        "codebases": codebases,
        "notes": [
            "The index records the source git commit at the time indexing completed.",
            "The index does not update automatically when an upstream source repository moves.",
            "To refresh a codebase, pull/fetch the source repository, reindex, validate, materialize, and validate materialized payloads.",
        ],
    }


def table_counts(con: sqlite3.Connection, codebase_id: int) -> dict[str, int]:
    counts = {}
    for table in COUNT_TABLES:
        if table == "materialized_tool_contexts":
            continue
        counts[table] = con.execute(
            f"SELECT COUNT(*) FROM {table} WHERE codebase_id=?", (codebase_id,)
        ).fetchone()[0]
    return counts


def materialized_counts(con: sqlite3.Connection, codebase_id: int) -> dict[str, int]:
    if not table_exists(con, "materialized_tool_contexts"):
        return {}
    rows = con.execute(
        """
        SELECT tool_name, COUNT(*) AS payloads
        FROM materialized_tool_contexts
        WHERE codebase_id=?
        GROUP BY tool_name
        ORDER BY tool_name
        """,
        (codebase_id,),
    ).fetchall()
    return {row["tool_name"]: row["payloads"] for row in rows}


def table_exists(con: sqlite3.Connection, table: str) -> bool:
    row = con.execute(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?", (table,)
    ).fetchone()
    return row[0] > 0


def pragma_scalar(con: sqlite3.Connection, name: str) -> int:
    return con.execute(f"PRAGMA {name}").fetchone()[0]


def render_markdown(inventory: dict[str, Any]) -> str:
    lines = [
        "# Indexed Codebases Inventory",
        "",
        "This file records which Odoo source versions are currently represented in the SQLite index.",
        "",
        "> The index does not update automatically when an upstream Git repository moves. Refresh requires pull/fetch, reindex, validate, materialize, and validate materialized payloads.",
        "",
        "## Database",
        "",
        f"- Path: `{inventory['database']['path']}`",
        f"- Page count: `{inventory['database']['page_count']}`",
        f"- Page size: `{inventory['database']['page_size']}`",
        f"- Freelist count: `{inventory['database']['freelist_count']}`",
        "",
        "## Codebases",
        "",
        "| Name | Series | Version | Branch | Commit | Indexed At | Modules | Symbols | Models | Fields | Methods | XML Records | Views | Materialized Payloads |",
        "|---|---:|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for cb in inventory["codebases"]:
        counts = cb["counts"]
        materialized_total = sum(cb.get("materialized", {}).values())
        lines.append(
            "| {name} | {series} | {version} | {branch} | {commit} | {indexed_at} | {modules} | {symbols} | {models} | {fields} | {methods} | {xml_records} | {views} | {materialized_total} |".format(
                name=cb.get("name") or "",
                series=cb.get("odoo_series") or "",
                version=cb.get("version") or "",
                branch=cb.get("git_branch") or "",
                commit=(cb.get("git_commit") or "")[:12],
                indexed_at=cb.get("indexed_at") or "",
                modules=counts.get("modules", 0),
                symbols=counts.get("symbols", 0),
                models=counts.get("models", 0),
                fields=counts.get("fields", 0),
                methods=counts.get("methods", 0),
                xml_records=counts.get("xml_records", 0),
                views=counts.get("views", 0),
                materialized_total=materialized_total,
            )
        )
    lines.extend(["", "## Materialized Payloads", ""])
    for cb in inventory["codebases"]:
        lines.append(f"### `{cb['name']}`")
        materialized = cb.get("materialized", {})
        if materialized:
            for tool, count in materialized.items():
                lines.append(f"- `{tool}`: `{count}`")
        else:
            lines.append("- none")
        lines.append("")
    lines.extend(["## Notes", ""])
    for note in inventory["notes"]:
        lines.append(f"- {note}")
    lines.append("")
    return "\n".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())
