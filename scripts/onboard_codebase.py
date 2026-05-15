#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

DEFAULT_ACCURACY_CASES = Path("benchmarks/accuracy/mcp_accuracy_cases.json")
DEFAULT_RUNS_DIR = Path("benchmarks/runs/onboarding")


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
        description="One-hit onboarding for a new indexed Odoo codebase version"
    )
    parser.add_argument("--name", required=True, help="Codebase name, for example odoo-20")
    parser.add_argument("--path", required=True, type=Path, help="Source checkout path")
    parser.add_argument("--config", required=True, type=Path, help="Odoo Knowledge config path")
    parser.add_argument(
        "--binary",
        default="odoo-knowledge-rs",
        help="Binary to execute; use target/release/odoo-knowledge for local testing",
    )
    parser.add_argument("--endpoint", help="Optional public/local MCP endpoint for accuracy and benchmark checks")
    parser.add_argument("--accuracy-cases", type=Path, default=DEFAULT_ACCURACY_CASES)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_RUNS_DIR)
    parser.add_argument("--benchmark-requests", type=int, default=40)
    parser.add_argument("--benchmark-concurrency", type=int, default=4)
    parser.add_argument("--materialized-validate-limit", type=int, default=30)
    parser.add_argument("--skip-add", action="store_true", help="Skip add-codebase when already registered")
    parser.add_argument("--skip-accuracy", action="store_true")
    parser.add_argument("--skip-benchmark", action="store_true")
    args = parser.parse_args()

    output_dir = args.output_dir / f"{time.strftime('%Y%m%dT%H%M%SZ')}-{safe_name(args.name)}"
    output_dir.mkdir(parents=True, exist_ok=True)

    steps: list[StepResult] = []
    failed = False

    def run_step(name: str, command: list[str], required: bool = True) -> None:
        nonlocal failed
        result = run_command(name, command)
        steps.append(result)
        write_step(output_dir, result)
        if required and not result.ok:
            failed = True

    base = [args.binary, "--config", str(args.config)]

    if not args.skip_add:
        run_step("add_codebase", base + ["add-codebase", "--name", args.name, "--path", str(args.path)])
    run_step("index", base + ["index", "--codebase", args.name])
    run_step("validate", base + ["validate", "--codebase", args.name])
    run_step("materialize_contexts", base + ["materialize-contexts", "--codebase", args.name])
    run_step(
        "validate_materialized",
        base
        + [
            "validate-materialized",
            "--codebase",
            args.name,
            "--limit",
            str(args.materialized_validate_limit),
        ],
    )

    if args.endpoint and not args.skip_accuracy:
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
                str(output_dir / "accuracy.json"),
            ],
        )

    if args.endpoint and not args.skip_benchmark:
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
                str(output_dir / "benchmark_light.json"),
            ],
        )

    report = {
        "codebase": args.name,
        "source_path": str(args.path),
        "config": str(args.config),
        "endpoint": args.endpoint,
        "output_dir": str(output_dir),
        "ok": not failed and all(step.ok for step in steps),
        "steps": [step_summary(step) for step in steps],
    }
    (output_dir / "onboarding_report.json").write_text(
        json.dumps(report, indent=2, sort_keys=True), encoding="utf-8"
    )
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["ok"] else 1


def run_command(name: str, command: list[str]) -> StepResult:
    started = time.perf_counter()
    completed = subprocess.run(command, text=True, capture_output=True)
    return StepResult(
        name=name,
        ok=completed.returncode == 0,
        elapsed_ms=(time.perf_counter() - started) * 1000,
        command=command,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def write_step(output_dir: Path, result: StepResult) -> None:
    payload = {
        "name": result.name,
        "ok": result.ok,
        "elapsed_ms": result.elapsed_ms,
        "command": result.command,
        "stdout": parse_json_or_text(result.stdout),
        "stderr": result.stderr,
    }
    (output_dir / f"{result.name}.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8"
    )


def step_summary(result: StepResult) -> dict[str, Any]:
    return {
        "name": result.name,
        "ok": result.ok,
        "elapsed_ms": result.elapsed_ms,
        "command": result.command,
    }


def parse_json_or_text(value: str) -> Any:
    text = value.strip()
    if not text:
        return ""
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return text


def safe_name(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "._-" else "-" for ch in value)


if __name__ == "__main__":
    raise SystemExit(main())
