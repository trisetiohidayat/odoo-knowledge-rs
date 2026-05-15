#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path
from typing import Any

QUERIES: list[dict[str, Any]] = [
    {
        "id": "search_exact_symbol_name",
        "sql": "SELECT kind, name, qualname, module, file_path FROM symbols WHERE codebase_id=? AND name=? LIMIT 10",
        "params": [1, "sale.order"],
        "expect_any": ["idx_symbols_codebase_name"],
    },
    {
        "id": "search_exact_symbol_qualname",
        "sql": "SELECT kind, name, qualname, module, file_path FROM symbols WHERE codebase_id=? AND qualname=? LIMIT 10",
        "params": [1, "method:sale.order.action_confirm"],
        "expect_any": ["idx_symbols_codebase_qualname"],
    },
    {
        "id": "model_context_contributors",
        "sql": "SELECT module, model_name, class_name FROM models WHERE codebase_id=? AND model_name=? ORDER BY module LIMIT 120",
        "params": [1, "sale.order"],
        "expect_any": ["idx_models_codebase_model_module_path", "idx_models_codebase_model"],
    },
    {
        "id": "field_context_definition",
        "sql": "SELECT * FROM fields WHERE codebase_id=? AND model_name=? AND field_name=? ORDER BY module, file_path, line_start",
        "params": [1, "product.template", "available_in_pos"],
        "expect_any": ["idx_fields_codebase_model_field_module_path", "idx_fields_codebase_model_field"],
    },
    {
        "id": "method_chain_exact",
        "sql": "SELECT * FROM methods WHERE codebase_id=? AND model_name=? AND method_name=?",
        "params": [1, "sale.order", "action_confirm"],
        "expect_any": ["idx_methods_codebase_model_method_module_path", "idx_methods_codebase_model_method"],
    },
    {
        "id": "xmlid_lookup_record",
        "sql": "SELECT * FROM xml_records WHERE codebase_id=? AND xmlid=?",
        "params": [1, "point_of_sale.product_template_form_view"],
        "expect_any": ["idx_xml_records_codebase_xmlid", "sqlite_autoindex_xml_records"],
    },
    {
        "id": "view_chain_xmlid",
        "sql": "SELECT * FROM views WHERE codebase_id=? AND xmlid=? ORDER BY module, xmlid",
        "params": [1, "point_of_sale.product_template_form_view"],
        "expect_any": ["idx_views_codebase_xmlid_module", "idx_views_codebase_xmlid"],
    },
    {
        "id": "module_context_manifest",
        "sql": "SELECT * FROM modules WHERE codebase_id=? AND name=?",
        "params": [1, "point_of_sale"],
        "expect_any": ["idx_modules_codebase_name", "sqlite_autoindex_modules"],
    },
    {
        "id": "materialized_context_lookup",
        "sql": "SELECT payload_json FROM materialized_tool_contexts WHERE codebase_id=? AND tool_name=? AND cache_key=? AND payload_version=? AND (source_indexed_at IS ? OR source_indexed_at = ?)",
        "params": [1, "odoo_xmlid_lookup", "point_of_sale.product_template_form_view", "mcp-context-v1", "2026-05-14", "2026-05-14"],
        "expect_any": ["sqlite_autoindex_materialized_tool_contexts", "idx_materialized_tool_contexts_lookup"],
    },
    {
        "id": "fts_symbols_lexical",
        "sql": "SELECT kind, name, qualname FROM fts_symbols WHERE codebase_id=? AND fts_symbols MATCH ? ORDER BY rank LIMIT 10",
        "params": [1, '"sale.order"'],
        "expect_any": ["VIRTUAL TABLE INDEX", "fts_symbols"],
    },
    {
        "id": "fts_chunks_lexical",
        "sql": "SELECT symbol_kind, symbol_name FROM fts_chunks WHERE codebase_id=? AND fts_chunks MATCH ? ORDER BY rank LIMIT 10",
        "params": [1, '"sale.order"'],
        "expect_any": ["VIRTUAL TABLE INDEX", "fts_chunks"],
    },
]


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit SQLite query plans for MCP hot paths")
    parser.add_argument("--db", type=Path, default=Path("data/index.db"))
    parser.add_argument("--output-json", type=Path)
    args = parser.parse_args()

    con = sqlite3.connect(args.db)
    results = [audit_query(con, query) for query in QUERIES]
    failed = sum(1 for result in results if not result["ok"])
    report = {"db": str(args.db), "queries": len(results), "failed": failed, "passed": len(results) - failed, "results": results}
    text = json.dumps(report, indent=2, sort_keys=True)
    if args.output_json:
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(text, encoding="utf-8")
    print(text)
    return 0 if failed == 0 else 1


def audit_query(con: sqlite3.Connection, query: dict[str, Any]) -> dict[str, Any]:
    rows = con.execute("EXPLAIN QUERY PLAN " + query["sql"], query["params"]).fetchall()
    details = [str(row[-1]) for row in rows]
    joined = "\n".join(details)
    ok = any(expected in joined for expected in query["expect_any"])
    return {
        "id": query["id"],
        "ok": ok,
        "expect_any": query["expect_any"],
        "plan": details,
    }


if __name__ == "__main__":
    raise SystemExit(main())
