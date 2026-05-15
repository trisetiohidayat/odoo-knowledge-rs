#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
import resource
import shutil
import sqlite3
import statistics
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

RUST_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PYTHON_ROOT = RUST_ROOT.parent / "odoo-knowledge"
DEFAULT_ODOO_ROOT = RUST_ROOT.parent / "odoo-19"
DEFAULT_RUNS_DIR = RUST_ROOT / "benchmarks" / "runs"

TABLES = [
    "codebases",
    "modules",
    "module_dependencies",
    "profiles",
    "profile_modules",
    "files",
    "symbols",
    "models",
    "fields",
    "methods",
    "xml_records",
    "views",
    "actions",
    "menus",
    "security_rules",
    "frontend_symbols",
    "graph_edges",
    "chunks",
    "index_diagnostics",
]

SCENARIOS = [
    {
        "id": "search_pos_product",
        "kind": "search",
        "args": ["search", "available in pos product", "--codebase", "odoo-19", "--module", "point_of_sale", "--limit", "5"],
    },
    {
        "id": "search_payment_screen",
        "kind": "search",
        "args": ["search", "PaymentScreen", "--codebase", "odoo-19", "--limit", "5"],
    },
    {
        "id": "tool_module_point_of_sale",
        "kind": "tool",
        "tool": "odoo_module_context",
        "arguments": {"module_name": "point_of_sale", "codebase": "odoo-19"},
    },
    {
        "id": "tool_model_product_template",
        "kind": "tool",
        "tool": "odoo_model_context",
        "arguments": {"model_name": "product.template", "codebase": "odoo-19"},
    },
    {
        "id": "tool_field_available_in_pos",
        "kind": "tool",
        "tool": "odoo_field_context",
        "arguments": {"model_name": "product.template", "field_name": "available_in_pos", "codebase": "odoo-19"},
    },
    {
        "id": "tool_method_sale_confirm",
        "kind": "tool",
        "tool": "odoo_method_chain",
        "arguments": {"model_name": "sale.order", "method_name": "action_confirm", "codebase": "odoo-19"},
    },
    {
        "id": "tool_view_sale_order_form",
        "kind": "tool",
        "tool": "odoo_view_chain",
        "arguments": {"xmlid_or_model": "sale.view_order_form", "codebase": "odoo-19"},
    },
    {
        "id": "tool_xmlid_sale_order_form",
        "kind": "tool",
        "tool": "odoo_xmlid_lookup",
        "arguments": {"xmlid": "sale.view_order_form", "codebase": "odoo-19"},
    },
]


@dataclass
class CommandResult:
    ok: bool
    command: list[str]
    elapsed_ms: float
    cpu_ms: float
    max_rss_kb: int
    stdout_bytes: int
    stderr_bytes: int
    returncode: int
    json_payload: Any | None = None
    error: str | None = None


@dataclass
class ScenarioSample:
    implementation: str
    scenario: str
    ok: bool
    elapsed_ms: float
    cpu_ms: float
    max_rss_kb: int
    stdout_bytes: int
    returncode: int
    metrics: dict[str, Any]
    error: str | None = None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Benchmark Python vs Rust Odoo Knowledge implementations.")
    parser.add_argument("--python-root", type=Path, default=DEFAULT_PYTHON_ROOT)
    parser.add_argument("--rust-root", type=Path, default=RUST_ROOT)
    parser.add_argument("--odoo-root", type=Path, default=DEFAULT_ODOO_ROOT)
    parser.add_argument("--codebase", default="odoo-19")
    parser.add_argument("--runs-dir", type=Path, default=DEFAULT_RUNS_DIR)
    parser.add_argument("--index-iterations", type=int, default=1)
    parser.add_argument("--warm-index-iterations", type=int, default=0)
    parser.add_argument("--query-iterations", type=int, default=5)
    parser.add_argument("--warmup-iterations", type=int, default=1)
    parser.add_argument("--thresholds-json", type=Path, help="Optional regression thresholds JSON file")
    parser.add_argument("--skip-rust-build", action="store_true")
    parser.add_argument("--keep-dbs", action="store_true")
    parser.add_argument("--only", choices=["python", "rust", "both"], default="both")
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--output-md", type=Path)
    args = parser.parse_args(argv)

    validate_args(args)
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = args.runs_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)

    output_json = args.output_json or run_dir / "benchmark.json"
    output_md = args.output_md or run_dir / "benchmark.md"
    python_db = run_dir / "python-index.db"
    rust_db = run_dir / "rust-index.db"

    if not args.skip_rust_build and args.only in {"rust", "both"}:
        print("Building Rust release binary...")
        run_checked(["bash", "-lc", '. "$HOME/.cargo/env" && cargo build --release -p odoo-knowledge-cli'], cwd=args.rust_root)

    implementations = []
    if args.only in {"python", "both"}:
        implementations.append(build_python_impl(args.python_root, python_db))
    if args.only in {"rust", "both"}:
        implementations.append(build_rust_impl(args.rust_root, rust_db))

    report: dict[str, Any] = {
        "run_id": run_id,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "environment": environment_info(args),
        "settings": {
            "codebase": args.codebase,
            "odoo_root": str(args.odoo_root),
            "index_iterations": args.index_iterations,
            "warm_index_iterations": args.warm_index_iterations,
            "query_iterations": args.query_iterations,
            "warmup_iterations": args.warmup_iterations,
            "run_dir": str(run_dir),
            "thresholds_json": str(args.thresholds_json) if args.thresholds_json else None,
        },
        "implementations": {},
        "comparisons": {},
    }

    for impl in implementations:
        print(f"Benchmarking {impl['name']}...")
        impl_report = benchmark_implementation(
            impl=impl,
            codebase=args.codebase,
            odoo_root=args.odoo_root,
            index_iterations=args.index_iterations,
            warm_index_iterations=args.warm_index_iterations,
            query_iterations=args.query_iterations,
            warmup_iterations=args.warmup_iterations,
        )
        report["implementations"][impl["name"]] = impl_report

    report["comparisons"] = compare_implementations(report["implementations"])
    report["thresholds"] = evaluate_thresholds(report, args.thresholds_json)
    report["finished_at"] = datetime.now(timezone.utc).isoformat()

    output_json.write_text(json.dumps(report, indent=2), encoding="utf-8")
    output_md.write_text(render_markdown(report), encoding="utf-8")
    print(f"Wrote JSON report: {output_json}")
    print(f"Wrote Markdown report: {output_md}")

    if not args.keep_dbs:
        for path in [python_db, rust_db]:
            remove_sqlite_files(path)

    return 0 if all_impls_ok(report["implementations"]) and report["thresholds"]["ok"] else 1


def validate_args(args: argparse.Namespace) -> None:
    if not args.python_root.exists() and args.only in {"python", "both"}:
        raise SystemExit(f"Python project not found: {args.python_root}")
    if not args.rust_root.exists() and args.only in {"rust", "both"}:
        raise SystemExit(f"Rust project not found: {args.rust_root}")
    if not args.odoo_root.exists():
        raise SystemExit(f"Odoo source root not found: {args.odoo_root}")
    if args.index_iterations < 1:
        raise SystemExit("--index-iterations must be >= 1")
    if args.warm_index_iterations < 0:
        raise SystemExit("--warm-index-iterations must be >= 0")
    if args.query_iterations < 1:
        raise SystemExit("--query-iterations must be >= 1")
    if args.warmup_iterations < 0:
        raise SystemExit("--warmup-iterations must be >= 0")
    if args.thresholds_json and not args.thresholds_json.exists():
        raise SystemExit(f"thresholds file not found: {args.thresholds_json}")


def build_python_impl(root: Path, db_path: Path) -> dict[str, Any]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(root)
    env["ODOO_KNOWLEDGE_DB"] = str(db_path)
    return {
        "name": "python",
        "root": root,
        "db_path": db_path,
        "env": env,
        "base_cmd": [sys.executable, "-m", "odoo_knowledge.cli", "--db", str(db_path)],
    }


def build_rust_impl(root: Path, db_path: Path) -> dict[str, Any]:
    binary = root / "target" / "release" / "odoo-knowledge"
    if not binary.exists():
        binary = root / "target" / "debug" / "odoo-knowledge"
    env = os.environ.copy()
    env["ODOO_KNOWLEDGE_DB"] = str(db_path)
    return {
        "name": "rust-tree-sitter",
        "root": root,
        "db_path": db_path,
        "env": env,
        "base_cmd": [str(binary), "--db", str(db_path)],
    }


def benchmark_implementation(
    impl: dict[str, Any],
    codebase: str,
    odoo_root: Path,
    index_iterations: int,
    warm_index_iterations: int,
    query_iterations: int,
    warmup_iterations: int,
) -> dict[str, Any]:
    db_path = impl["db_path"]
    index_runs = []

    for idx in range(index_iterations):
        remove_sqlite_files(db_path)
        add_result = run_json(
            impl["base_cmd"] + ["add-codebase", "--name", codebase, "--path", str(odoo_root)],
            cwd=impl["root"],
            env=impl["env"],
        )

    warm_index_runs = []
    for idx in range(warm_index_iterations):
        index_result = run_json(
            impl["base_cmd"] + ["index", "--codebase", codebase],
            cwd=impl["root"],
            env=impl["env"],
        )
        warm_index_runs.append(
            {
                "iteration": idx + 1,
                "index": asdict(index_result),
                "table_counts": table_counts(db_path),
                "db_bytes": db_size_bytes(db_path),
            }
        )
        index_result = run_json(
            impl["base_cmd"] + ["index", "--codebase", codebase],
            cwd=impl["root"],
            env=impl["env"],
        )
        counts = table_counts(db_path)
        index_runs.append(
            {
                "iteration": idx + 1,
                "add_codebase": asdict(add_result),
                "index": asdict(index_result),
                "table_counts": counts,
                "db_bytes": db_size_bytes(db_path),
            }
        )

    warmup_samples: list[ScenarioSample] = []
    for scenario in SCENARIOS:
        for _ in range(warmup_iterations):
            warmup_samples.append(run_scenario(impl, scenario, codebase))

    scenario_samples: list[ScenarioSample] = []
    for scenario in SCENARIOS:
        for _ in range(query_iterations):
            scenario_samples.append(run_scenario(impl, scenario, codebase))

    grouped = summarize_samples(scenario_samples)
    return {
        "ok": all(run["index"]["ok"] for run in index_runs) and all(sample.ok for sample in scenario_samples),
        "db_path": str(db_path),
        "index_runs": index_runs,
        "warm_index_runs": warm_index_runs,
        "latest_table_counts": table_counts(db_path),
        "latest_db_bytes": db_size_bytes(db_path),
        "scenario_summary": grouped,
        "warmup_samples": [asdict(sample) for sample in warmup_samples],
        "scenario_samples": [asdict(sample) for sample in scenario_samples],
    }


def run_scenario(impl: dict[str, Any], scenario: dict[str, Any], codebase: str) -> ScenarioSample:
    if scenario["kind"] == "search":
        cmd = impl["base_cmd"] + replace_codebase_args(scenario["args"], codebase)
    else:
        arguments = replace_codebase_value(scenario["arguments"], codebase)
        cmd = impl["base_cmd"] + [
            "tool",
            scenario["tool"],
            json.dumps(arguments, separators=(",", ":")),
        ]
    result = run_json(cmd, cwd=impl["root"], env=impl["env"])
    return ScenarioSample(
        implementation=impl["name"],
        scenario=scenario["id"],
        ok=result.ok,
        elapsed_ms=result.elapsed_ms,
        cpu_ms=result.cpu_ms,
        max_rss_kb=result.max_rss_kb,
        stdout_bytes=result.stdout_bytes,
        returncode=result.returncode,
        metrics=payload_metrics(scenario, result.json_payload),
        error=result.error,
    )

def replace_codebase_args(args: list[str], codebase: str) -> list[str]:
    result = list(args)
    for idx, value in enumerate(result[:-1]):
        if value == "--codebase":
            result[idx + 1] = codebase
    return result

def replace_codebase_value(arguments: dict[str, Any], codebase: str) -> dict[str, Any]:
    result = dict(arguments)
    if "codebase" in result:
        result["codebase"] = codebase
    return result


def run_json(cmd: list[str], cwd: Path, env: dict[str, str]) -> CommandResult:
    usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    time_bin = shutil.which("/usr/bin/time") or shutil.which("time")
    actual_cmd = [time_bin, "-f", "__ODOO_BENCH_RSS_KB__%M", *cmd] if time_bin else cmd
    start = time.perf_counter()
    proc = subprocess.run(actual_cmd, cwd=cwd, env=env, text=True, capture_output=True)
    elapsed = (time.perf_counter() - start) * 1000
    usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu_ms = (
        usage_after.ru_utime
        - usage_before.ru_utime
        + usage_after.ru_stime
        - usage_before.ru_stime
    ) * 1000
    stderr, max_rss_kb = parse_time_stderr(proc.stderr)
    payload = None
    error = None
    ok = proc.returncode == 0
    if proc.stdout.strip():
        try:
            payload = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            ok = False
            error = f"invalid JSON stdout: {exc}"
    if proc.returncode != 0:
        error = (stderr or proc.stdout).strip()[:2000]
    return CommandResult(
        ok=ok,
        command=cmd,
        elapsed_ms=elapsed,
        cpu_ms=cpu_ms,
        max_rss_kb=max_rss_kb,
        stdout_bytes=len(proc.stdout.encode("utf-8")),
        stderr_bytes=len(stderr.encode("utf-8")),
        returncode=proc.returncode,
        json_payload=payload,
        error=error,
    )

def parse_time_stderr(stderr: str) -> tuple[str, int]:
    marker = "__ODOO_BENCH_RSS_KB__"
    cleaned = []
    max_rss_kb = 0
    for line in stderr.splitlines():
        if line.startswith(marker):
            try:
                max_rss_kb = int(line[len(marker):].strip())
            except ValueError:
                max_rss_kb = 0
        else:
            cleaned.append(line)
    return "\n".join(cleaned), max_rss_kb


def run_checked(cmd: list[str], cwd: Path) -> None:
    proc = subprocess.run(cmd, cwd=cwd, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"command failed: {' '.join(cmd)}")


def table_counts(db_path: Path) -> dict[str, int | None]:
    if not db_path.exists():
        return {table: None for table in TABLES}
    con = sqlite3.connect(db_path)
    try:
        counts: dict[str, int | None] = {}
        for table in TABLES:
            try:
                counts[table] = int(con.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])
            except sqlite3.Error:
                counts[table] = None
        return counts
    finally:
        con.close()


def db_size_bytes(db_path: Path) -> int:
    total = 0
    for suffix in ["", "-wal", "-shm"]:
        path = Path(str(db_path) + suffix)
        if path.exists():
            total += path.stat().st_size
    return total


def remove_sqlite_files(db_path: Path) -> None:
    for suffix in ["", "-wal", "-shm"]:
        path = Path(str(db_path) + suffix)
        if path.exists():
            path.unlink()


def payload_metrics(scenario: dict[str, Any], payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        return {}
    sid = scenario["id"]
    if scenario["kind"] == "search":
        results = payload.get("results") or {}
        return {
            "symbols": len(results.get("symbols") or []),
            "chunks": len(results.get("chunks") or []),
        }
    if sid == "tool_module_point_of_sale":
        return {
            "depends": len(payload.get("depends") or []),
            "dependents": len(payload.get("dependents") or []),
            "models": len(payload.get("models") or []),
            "views": len(payload.get("views") or []),
        }
    if sid == "tool_model_product_template":
        return {
            "contributors": len(payload.get("contributors") or []),
            "fields": len(payload.get("fields") or []),
            "methods": len(payload.get("methods") or []),
            "views": len(payload.get("views") or []),
        }
    if sid == "tool_field_available_in_pos":
        return {
            "definitions": len(payload.get("definitions") or []),
            "related_views_sample": len(payload.get("related_views_sample") or []),
        }
    if sid == "tool_method_sale_confirm":
        return {"chain": len(payload.get("chain") or [])}
    if sid == "tool_view_sale_order_form":
        return {"views": len(payload.get("views") or [])}
    if sid == "tool_xmlid_sale_order_form":
        return {
            "records": len(payload.get("records") or []),
            "views": len(payload.get("views") or []),
            "actions": len(payload.get("actions") or []),
            "menus": len(payload.get("menus") or []),
        }
    return {}


def summarize_samples(samples: list[ScenarioSample]) -> dict[str, Any]:
    grouped: dict[str, list[ScenarioSample]] = {}
    for sample in samples:
        grouped.setdefault(sample.scenario, []).append(sample)
    return {name: summarize_group(rows) for name, rows in grouped.items()}


def summarize_group(rows: list[ScenarioSample]) -> dict[str, Any]:
    latencies = [row.elapsed_ms for row in rows if row.ok]
    cpu_values = [row.cpu_ms for row in rows if row.ok]
    rss_values = [row.max_rss_kb for row in rows if row.ok]
    return {
        "iterations": len(rows),
        "ok": sum(1 for row in rows if row.ok),
        "failed": sum(1 for row in rows if not row.ok),
        "latency_ms": {
            "min": min(latencies) if latencies else None,
            "mean": statistics.mean(latencies) if latencies else None,
            "median": statistics.median(latencies) if latencies else None,
            "p95": percentile(latencies, 95),
            "p99": percentile(latencies, 99),
            "max": max(latencies) if latencies else None,
        },
        "cpu_ms": {
            "mean": statistics.mean(cpu_values) if cpu_values else None,
            "p95": percentile(cpu_values, 95),
            "max": max(cpu_values) if cpu_values else None,
        },
        "max_rss_kb": {
            "mean": statistics.mean(rss_values) if rss_values else None,
            "max": max(rss_values) if rss_values else None,
        },
        "stdout_bytes_mean": statistics.mean([row.stdout_bytes for row in rows]) if rows else 0,
        "last_metrics": rows[-1].metrics if rows else {},
        "errors": [row.error for row in rows if row.error][:3],
    }

def percentile(values: list[float], percentile_value: int) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * percentile_value / 100
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight

def evaluate_thresholds(report: dict[str, Any], thresholds_path: Path | None) -> dict[str, Any]:
    if thresholds_path is None:
        return {"ok": True, "enabled": False, "failures": []}
    thresholds = json.loads(thresholds_path.read_text(encoding="utf-8"))
    failures = []
    for impl_name, impl_thresholds in thresholds.get("implementations", {}).items():
        impl = report["implementations"].get(impl_name)
        if not impl:
            failures.append({"implementation": impl_name, "error": "implementation missing"})
            continue
        max_index_ms = impl_thresholds.get("max_mean_index_ms")
        actual_index_ms = mean_index_ms(impl)
        if max_index_ms is not None and actual_index_ms is not None and actual_index_ms > max_index_ms:
            failures.append({"implementation": impl_name, "metric": "mean_index_ms", "actual": actual_index_ms, "threshold": max_index_ms})
        for scenario, scenario_thresholds in impl_thresholds.get("scenarios", {}).items():
            summary = impl.get("scenario_summary", {}).get(scenario)
            if not summary:
                failures.append({"implementation": impl_name, "scenario": scenario, "error": "scenario missing"})
                continue
            checks = {
                "max_mean_ms": summary["latency_ms"].get("mean"),
                "max_p95_ms": summary["latency_ms"].get("p95"),
                "max_p99_ms": summary["latency_ms"].get("p99"),
                "max_cpu_mean_ms": summary["cpu_ms"].get("mean"),
                "max_rss_kb": summary["max_rss_kb"].get("max"),
            }
            for threshold_name, actual in checks.items():
                threshold = scenario_thresholds.get(threshold_name)
                if threshold is not None and actual is not None and actual > threshold:
                    failures.append({"implementation": impl_name, "scenario": scenario, "metric": threshold_name, "actual": actual, "threshold": threshold})
    return {"ok": not failures, "enabled": True, "path": str(thresholds_path), "failures": failures}


def compare_implementations(implementations: dict[str, Any]) -> dict[str, Any]:
    if "python" not in implementations or "rust-tree-sitter" not in implementations:
        return {}
    py = implementations["python"]
    rs = implementations["rust-tree-sitter"]
    py_index = mean_index_ms(py)
    rs_index = mean_index_ms(rs)
    return {
        "index_speedup_rust_vs_python": safe_ratio(py_index, rs_index),
        "db_size_ratio_rust_vs_python": safe_ratio(rs.get("latest_db_bytes"), py.get("latest_db_bytes")),
        "table_count_ratios_rust_vs_python": {
            table: safe_ratio(
                rs["latest_table_counts"].get(table),
                py["latest_table_counts"].get(table),
            )
            for table in TABLES
        },
        "scenario_latency_speedups_rust_vs_python": scenario_speedups(py, rs),
    }


def mean_index_ms(report: dict[str, Any]) -> float | None:
    values = [run["index"]["elapsed_ms"] for run in report.get("index_runs", []) if run["index"]["ok"]]
    return statistics.mean(values) if values else None


def scenario_speedups(py: dict[str, Any], rs: dict[str, Any]) -> dict[str, float | None]:
    result = {}
    for scenario in sorted(set(py["scenario_summary"]) & set(rs["scenario_summary"])):
        py_mean = py["scenario_summary"][scenario]["latency_ms"]["mean"]
        rs_mean = rs["scenario_summary"][scenario]["latency_ms"]["mean"]
        result[scenario] = safe_ratio(py_mean, rs_mean)
    return result


def safe_ratio(left: Any, right: Any) -> float | None:
    if left is None or right in (None, 0):
        return None
    return float(left) / float(right)


def environment_info(args: argparse.Namespace) -> dict[str, Any]:
    rust_version = command_text(["bash", "-lc", '. "$HOME/.cargo/env" && rustc --version'], cwd=args.rust_root)
    cargo_version = command_text(["bash", "-lc", '. "$HOME/.cargo/env" && cargo --version'], cwd=args.rust_root)
    python_version = command_text([sys.executable, "--version"], cwd=args.python_root if args.python_root.exists() else args.rust_root)
    return {
        "platform": platform.platform(),
        "python": python_version,
        "rustc": rust_version,
        "cargo": cargo_version,
        "python_root": str(args.python_root),
        "rust_root": str(args.rust_root),
    }


def command_text(cmd: list[str], cwd: Path) -> str | None:
    try:
        proc = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, timeout=10)
    except Exception:
        return None
    return (proc.stdout or proc.stderr).strip() or None


def all_impls_ok(implementations: dict[str, Any]) -> bool:
    return all(report.get("ok") for report in implementations.values())


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Python vs Rust Benchmark",
        "",
        f"- Run: `{report['run_id']}`",
        f"- Odoo root: `{report['settings']['odoo_root']}`",
        f"- Index iterations: `{report['settings']['index_iterations']}`",
        f"- Warm index iterations: `{report['settings']['warm_index_iterations']}`",
        f"- Query iterations: `{report['settings']['query_iterations']}`",
        f"- Warmup iterations: `{report['settings']['warmup_iterations']}`",
        "",
        "## Index",
        "",
        "| Implementation | OK | Mean index ms | DB MB | modules | files | models | fields | methods | xml_records | views | frontend | symbols | diagnostics |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for name, impl in report["implementations"].items():
        counts = impl["latest_table_counts"]
        lines.append(
            "| {name} | {ok} | {index_ms} | {db_mb} | {modules} | {files} | {models} | {fields} | {methods} | {xml_records} | {views} | {frontend} | {symbols} | {diagnostics} |".format(
                name=name,
                ok="yes" if impl["ok"] else "no",
                index_ms=fmt(mean_index_ms(impl)),
                db_mb=fmt(impl["latest_db_bytes"] / 1024 / 1024 if impl["latest_db_bytes"] else None),
                modules=counts.get("modules"),
                files=counts.get("files"),
                models=counts.get("models"),
                fields=counts.get("fields"),
                methods=counts.get("methods"),
                xml_records=counts.get("xml_records"),
                views=counts.get("views"),
                frontend=counts.get("frontend_symbols"),
                symbols=counts.get("symbols"),
                diagnostics=counts.get("index_diagnostics"),
            )
        )
    lines.extend(["", "## Scenario Latency", ""])
    for name, impl in report["implementations"].items():
        lines.extend([
            f"### {name}",
            "",
            "| Scenario | OK | Mean ms | Median ms | P95 ms | P99 ms | Max ms | CPU mean ms | Max RSS KB | Mean bytes | Last metrics |",
            "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|",
        ])
        for scenario, summary in impl["scenario_summary"].items():
            lat = summary["latency_ms"]
            cpu = summary["cpu_ms"]
            rss = summary["max_rss_kb"]
            lines.append(
                f"| `{scenario}` | {summary['ok']}/{summary['iterations']} | {fmt(lat['mean'])} | {fmt(lat['median'])} | {fmt(lat['p95'])} | {fmt(lat['p99'])} | {fmt(lat['max'])} | {fmt(cpu['mean'])} | {fmt(rss['max'])} | {fmt(summary['stdout_bytes_mean'])} | `{json.dumps(summary['last_metrics'], sort_keys=True)}` |"
            )
        lines.append("")
    if report.get("thresholds", {}).get("enabled"):
        thresholds = report["thresholds"]
        lines.extend(["## Thresholds", "", f"- OK: `{'yes' if thresholds['ok'] else 'no'}`", f"- File: `{thresholds.get('path')}`", ""])
        if thresholds.get("failures"):
            lines.extend(["| Implementation | Scenario | Metric | Actual | Threshold |", "|---|---|---|---:|---:|"])
            for failure in thresholds["failures"]:
                lines.append(
                    f"| `{failure.get('implementation', '-')}` | `{failure.get('scenario', '-')}` | `{failure.get('metric', failure.get('error', '-'))}` | {fmt(failure.get('actual'))} | {fmt(failure.get('threshold'))} |"
                )
            lines.append("")
    if report.get("comparisons"):
        comp = report["comparisons"]
        lines.extend([
            "## Comparison",
            "",
            f"- Rust index speedup vs Python: `{fmt(comp.get('index_speedup_rust_vs_python'))}x`",
            f"- Rust DB size ratio vs Python: `{fmt(comp.get('db_size_ratio_rust_vs_python'))}`",
            "",
            "| Scenario | Rust latency speedup vs Python |",
            "|---|---:|",
        ])
        for scenario, value in comp.get("scenario_latency_speedups_rust_vs_python", {}).items():
            lines.append(f"| `{scenario}` | {fmt(value)}x |")
        lines.append("")
    return "\n".join(lines) + "\n"


def fmt(value: Any) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        return f"{value:.2f}"
    return str(value)


if __name__ == "__main__":
    raise SystemExit(main())
