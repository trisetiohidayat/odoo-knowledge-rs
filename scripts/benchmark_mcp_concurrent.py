#!/usr/bin/env python3
from __future__ import annotations

import argparse
import concurrent.futures
import json
import statistics
import time
import urllib.request
from pathlib import Path
from typing import Any

DEFAULT_REQUESTS = [
    {"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}},
    {"jsonrpc": "2.0", "id": 2, "method": "ping", "params": {}},
]

def main() -> int:
    parser = argparse.ArgumentParser(description="Concurrent HTTP JSON-RPC MCP benchmark")
    parser.add_argument("--endpoint", default="http://127.0.0.1:8765/mcp")
    parser.add_argument("--bearer-token")
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--requests", type=int, default=100)
    parser.add_argument("--output-json", type=Path)
    args = parser.parse_args()

    payloads = [DEFAULT_REQUESTS[idx % len(DEFAULT_REQUESTS)] for idx in range(args.requests)]
    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        samples = list(executor.map(lambda payload: call(args.endpoint, args.bearer_token, payload), payloads))
    elapsed = (time.perf_counter() - started) * 1000
    latencies = [sample["elapsed_ms"] for sample in samples if sample["ok"]]
    report = {
        "endpoint": args.endpoint,
        "concurrency": args.concurrency,
        "requests": args.requests,
        "ok": sum(1 for sample in samples if sample["ok"]),
        "failed": sum(1 for sample in samples if not sample["ok"]),
        "elapsed_ms": elapsed,
        "latency_ms": {
            "mean": statistics.mean(latencies) if latencies else None,
            "median": statistics.median(latencies) if latencies else None,
            "p95": percentile(latencies, 95),
            "p99": percentile(latencies, 99),
            "max": max(latencies) if latencies else None,
        },
        "samples": samples[:20],
    }
    text = json.dumps(report, indent=2)
    if args.output_json:
        args.output_json.write_text(text, encoding="utf-8")
    print(text)
    return 0 if report["failed"] == 0 else 1

def call(endpoint: str, token: str | None, payload: dict[str, Any]) -> dict[str, Any]:
    body = json.dumps(payload).encode()
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    started = time.perf_counter()
    try:
        request = urllib.request.Request(endpoint, data=body, headers=headers, method="POST")
        with urllib.request.urlopen(request, timeout=30) as response:
            response_body = response.read()
        return {"ok": True, "status": response.status, "elapsed_ms": (time.perf_counter() - started) * 1000, "bytes": len(response_body)}
    except Exception as exc:
        return {"ok": False, "elapsed_ms": (time.perf_counter() - started) * 1000, "error": str(exc)}

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

if __name__ == "__main__":
    raise SystemExit(main())
