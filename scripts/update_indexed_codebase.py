#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

DEFAULT_RUNS_DIR = Path("benchmarks/runs/updates")
DEFAULT_ACCURACY_CASES = Path("benchmarks/accuracy/mcp_accuracy_cases.json")


@dataclass
class StepResult:
    name: str
    ok: bool
    elapsed_ms: float
    command: list[str]
    stdout: str
    stderr: str


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Guarded one-hit refresh for an already indexed Odoo codebase"
    )
    parser.add_argument("--codebase", required=True, help="Existing codebase name, for example odoo-19")
    parser.add_argument("--source-path", required=True, type=Path, help="Git checkout for this codebase")
    parser.add_argument("--config", required=True, type=Path, help="Odoo Knowledge config path")
    parser.add_argument(
        "--binary",
        default="odoo-knowledge-rs",
        help="Binary to execute; use target/release/odoo-knowledge for local testing",
    )
    parser.add_argument("--endpoint", help="Optional public/local MCP endpoint for accuracy and benchmark checks")
    parser.add_argument("--accuracy-cases", type=Path, default=DEFAULT_ACCURACY_CASES)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_RUNS_DIR)
    parser.add_argument("--materialized-validate-limit", type=int, default=30)
    parser.add_argument("--benchmark-requests", type=int, default=40)
    parser.add_argument("--benchmark-concurrency", type=int, default=4)
    parser.add_argument("--apply", action="store_true", help="Actually fetch/pull/reindex. Default is dry-run only.")
    parser.add_argument("--allow-dirty", action="store_true", help="Allow updating a source checkout with local changes")
    parser.add_argument("--skip-materialize", action="store_true")
    parser.add_argument("--skip-accuracy", action="store_true")
    parser.add_argument("--skip-benchmark", action="store_true")
    parser.add_argument("--inventory-db", type=Path, help="Optional SQLite DB path for post-update inventory export")
    parser.add_argument("--restart-command", help="Optional command run after successful apply, for example a service restart")
    args = parser.parse_args()

    source_path = args.source_path.resolve()
    if not source_path.exists():
        print(json.dumps({"ok": False, "error": f"source path not found: {source_path}"}, indent=2))
        return 2
    if not (source_path / ".git").exists():
        print(json.dumps({"ok": False, "error": f"source path is not a git checkout: {source_path}"}, indent=2))
        return 2

    run_dir = args.output_dir / f"{time.strftime('%Y%m%dT%H%M%SZ')}-{safe_name(args.codebase)}"
    run_dir.mkdir(parents=True, exist_ok=True)

    plan = build_plan(args, source_path)
    before = git_state(source_path)
    report: dict[str, Any] = {
        "codebase": args.codebase,
        "mode": "apply" if args.apply else "dry-run",
        "source_path": redact_path(source_path),
        "config": redact_path(args.config),
        "endpoint": args.endpoint,
        "output_dir": str(run_dir),
        "before": before,
        "planned_steps": plan,
        "notes": [
            "Default mode is dry-run and does not mutate the source checkout or SQLite index.",
            "Use --apply only after reviewing the plan and ensuring backups/snapshots exist.",
            "This script never derives source paths from the database to avoid leaking server paths into reports.",
        ],
    }

    if not args.apply:
        report["ok"] = True
        report["after"] = before
        write_json(run_dir / "update_report.json", report)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0

    if before.get("dirty") and not args.allow_dirty:
        report["ok"] = False
        report["error"] = "source checkout has local changes; commit/stash them or pass --allow-dirty"
        write_json(run_dir / "update_report.json", report)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 2

    steps: list[StepResult] = []
    failed = False

    def run_step(name: str, command: list[str], cwd: Path | None = None, shell: bool = False) -> None:
        nonlocal failed
        result = run_command(name, command, cwd=cwd, shell=shell)
        steps.append(result)
        write_step(run_dir, result)
        if not result.ok:
            failed = True

    run_step("git_fetch", ["git", "fetch", "--prune", "origin"], cwd=source_path)
    if not failed:
        run_step("git_pull_ff_only", ["git", "pull", "--ff-only"], cwd=source_path)

    base = [args.binary, "--config", str(args.config)]
    if not failed:
        run_step("index", base + ["index", "--codebase", args.codebase])
    if not failed:
        run_step("validate", base + ["validate", "--codebase", args.codebase])
    if not failed and not args.skip_materialize:
        run_step("materialize_contexts", base + ["materialize-contexts", "--codebase", args.codebase])
    if not failed and not args.skip_materialize:
        run_step(
            "validate_materialized",
            base + ["validate-materialized", "--codebase", args.codebase, "--limit", str(args.materialized_validate_limit)],
        )
    if not failed and args.endpoint and not args.skip_accuracy:
        run_step(
            "accuracy",
            [
                sys.executable,
                "scripts/evaluate_mcp_accuracy.py",
                "--endpoint",
                args.endpoint,
                "--cases",
                str(args.accuracy_cases),
                "--output-json",
                str(run_dir / "accuracy.json"),
            ],
        )
    if not failed and args.endpoint and not args.skip_benchmark:
        run_step(
            "benchmark_light",
            [
                sys.executable,
                "scripts/benchmark_mcp_concurrent.py",
                "--endpoint",
                args.endpoint,
                "--concurrency",
                str(args.benchmark_concurrency),
                "--requests",
                str(args.benchmark_requests),
                "--output-json",
                str(run_dir / "benchmark_light.json"),
            ],
        )
    if not failed and args.inventory_db:
        run_step(
            "export_inventory",
            [
                sys.executable,
                "scripts/export_index_inventory.py",
                "--db",
                str(args.inventory_db),
                "--output-json",
                str(run_dir / "inventory.json"),
                "--output-md",
                str(run_dir / "inventory.md"),
            ],
        )
    if not failed and args.restart_command:
        run_step("restart", [args.restart_command], shell=True)

    report["after"] = git_state(source_path)
    report["ok"] = not failed and all(step.ok for step in steps)
    report["steps"] = [step_summary(step) for step in steps]
    write_json(run_dir / "update_report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["ok"] else 1


def build_plan(args: argparse.Namespace, source_path: Path) -> list[dict[str, Any]]:
    base = [args.binary, "--config", str(args.config)]
    steps = [
        {"name": "inspect_git", "mutates": False, "command": ["git", "status", "--short"]},
        {"name": "git_fetch", "mutates": True, "command": ["git", "fetch", "--prune", "origin"]},
        {"name": "git_pull_ff_only", "mutates": True, "command": ["git", "pull", "--ff-only"]},
        {"name": "index", "mutates": True, "command": base + ["index", "--codebase", args.codebase]},
        {"name": "validate", "mutates": False, "command": base + ["validate", "--codebase", args.codebase]},
    ]
    if not args.skip_materialize:
        steps.extend(
            [
                {"name": "materialize_contexts", "mutates": True, "command": base + ["materialize-contexts", "--codebase", args.codebase]},
                {
                    "name": "validate_materialized",
                    "mutates": False,
                    "command": base + ["validate-materialized", "--codebase", args.codebase, "--limit", str(args.materialized_validate_limit)],
                },
            ]
        )
    if args.endpoint and not args.skip_accuracy:
        steps.append({"name": "accuracy", "mutates": False, "command": [sys.executable, "scripts/evaluate_mcp_accuracy.py", "--endpoint", args.endpoint]})
    if args.endpoint and not args.skip_benchmark:
        steps.append({"name": "benchmark_light", "mutates": False, "command": [sys.executable, "scripts/benchmark_mcp_concurrent.py", "--endpoint", args.endpoint]})
    if args.inventory_db:
        steps.append({"name": "export_inventory", "mutates": False, "command": [sys.executable, "scripts/export_index_inventory.py", "--db", str(args.inventory_db)]})
    if args.restart_command:
        steps.append({"name": "restart", "mutates": True, "command": [args.restart_command]})
    for step in steps:
        step["cwd"] = redact_path(source_path) if step["name"].startswith("git_") or step["name"] == "inspect_git" else None
    return steps


def git_state(source_path: Path) -> dict[str, Any]:
    def git(args: list[str]) -> str:
        completed = subprocess.run(["git", *args], cwd=source_path, text=True, capture_output=True)
        return completed.stdout.strip() if completed.returncode == 0 else ""

    status = git(["status", "--short"])
    return {
        "branch": git(["branch", "--show-current"]),
        "head": git(["rev-parse", "HEAD"]),
        "upstream": git(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]),
        "upstream_head_local_ref": git(["rev-parse", "@{u}"]),
        "dirty": bool(status),
        "status_short": status.splitlines(),
    }


def run_command(name: str, command: list[str], cwd: Path | None = None, shell: bool = False) -> StepResult:
    started = time.perf_counter()
    if shell:
        completed = subprocess.run(command[0], text=True, capture_output=True, shell=True)
    else:
        completed = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    return StepResult(
        name=name,
        ok=completed.returncode == 0,
        elapsed_ms=(time.perf_counter() - started) * 1000,
        command=command,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def write_step(output_dir: Path, result: StepResult) -> None:
    write_json(
        output_dir / f"{result.name}.json",
        {
            "name": result.name,
            "ok": result.ok,
            "elapsed_ms": result.elapsed_ms,
            "command": result.command,
            "stdout": parse_json_or_text(result.stdout),
            "stderr": result.stderr,
        },
    )


def step_summary(result: StepResult) -> dict[str, Any]:
    return {"name": result.name, "ok": result.ok, "elapsed_ms": result.elapsed_ms, "command": result.command}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def parse_json_or_text(value: str) -> Any:
    text = value.strip()
    if not text:
        return ""
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return text


def redact_path(path: Path) -> str:
    marker = os.environ.get("ODOO_KNOWLEDGE_REPORT_PATHS", "redacted")
    return str(path) if marker == "include" else "<REDACTED>"


def safe_name(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "._-" else "-" for ch in value)


if __name__ == "__main__":
    raise SystemExit(main())
