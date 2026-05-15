#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import time
import urllib.request
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser(description="Evaluate MCP tool accuracy against JSON fixtures")
    parser.add_argument("--endpoint", default="http://127.0.0.1:8766/mcp")
    parser.add_argument("--cases", type=Path, default=Path("benchmarks/accuracy/mcp_accuracy_cases.json"))
    parser.add_argument("--output-json", type=Path)
    parser.add_argument("--bearer-token")
    args = parser.parse_args()

    suite = json.loads(args.cases.read_text(encoding="utf-8"))
    results = [run_case(args.endpoint, args.bearer_token, idx + 1, case) for idx, case in enumerate(suite["cases"])]
    passed = sum(1 for result in results if result["ok"])
    ranking = [metric for result in results for metric in result.get("ranking", [])]
    top1 = sum(1 for metric in ranking if metric["rank"] == 1)
    top5 = sum(1 for metric in ranking if metric["rank"] is not None and metric["rank"] <= 5)
    reciprocal_ranks = [1 / metric["rank"] if metric["rank"] else 0 for metric in ranking]
    report = {
        "endpoint": args.endpoint,
        "cases": len(results),
        "passed": passed,
        "failed": len(results) - passed,
        "ranking_metrics": {
            "checks": len(ranking),
            "top1_accuracy": top1 / len(ranking) if ranking else None,
            "top5_recall": top5 / len(ranking) if ranking else None,
            "mrr": sum(reciprocal_ranks) / len(reciprocal_ranks) if reciprocal_ranks else None,
            "ndcg_at_5": sum(ndcg_at_5(metric["rank"]) for metric in ranking) / len(ranking) if ranking else None,
        },
        "results": results,
    }
    text = json.dumps(report, indent=2, sort_keys=True)
    if args.output_json:
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(text, encoding="utf-8")
    print(text)
    return 0 if report["failed"] == 0 else 1


def run_case(endpoint: str, token: str | None, request_id: int, case: dict[str, Any]) -> dict[str, Any]:
    payload = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {"name": case["tool"], "arguments": case.get("arguments", {})},
    }
    started = time.perf_counter()
    try:
        response = call(endpoint, token, payload)
        tool_payload = decode_tool_payload(response)
        failures, ranking = evaluate_expectations(tool_payload, case.get("expect", {}))
        return {
            "id": case["id"],
            "tool": case["tool"],
            "ok": not failures,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            "ranking": ranking,
            "failures": failures,
        }
    except Exception as exc:
        return {
            "id": case["id"],
            "tool": case["tool"],
            "ok": False,
            "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
            "failures": [str(exc)],
        }


def call(endpoint: str, token: str | None, payload: dict[str, Any]) -> dict[str, Any]:
    headers = {"Content-Type": "application/json"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(
        endpoint,
        data=json.dumps(payload).encode(),
        headers=headers,
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def decode_tool_payload(response: dict[str, Any]) -> Any:
    if "error" in response:
        raise RuntimeError(f"JSON-RPC error: {response['error']}")
    content = response.get("result", {}).get("content", [])
    if not content:
        raise RuntimeError("missing MCP content")
    text = content[0].get("text")
    if not isinstance(text, str):
        raise RuntimeError("missing MCP text content")
    return json.loads(text)


def evaluate_expectations(payload: Any, expect: dict[str, Any]) -> tuple[list[str], list[dict[str, Any]]]:
    failures: list[str] = []
    ranking: list[dict[str, Any]] = []
    compact = json.dumps(payload, sort_keys=True, separators=(",", ":"))

    for needle in expect.get("contains", []):
        if needle not in compact:
            failures.append(f"expected response to contain {needle!r}")

    for item in expect.get("equals", []):
        value = get_path(payload, item["path"])
        if value != item["value"]:
            failures.append(f"expected {item['path']} == {item['value']!r}, got {value!r}")

    for item in expect.get("min_counts", []):
        value = get_path(payload, item["path"])
        actual = len(value) if isinstance(value, list) else 0
        if actual < item["count"]:
            failures.append(f"expected {item['path']} count >= {item['count']}, got {actual}")

    for item in expect.get("ranking", []):
        values = get_path(payload, item["path"])
        rank = rank_contains(values, item["contains"])
        ranking.append({"path": item["path"], "contains": item["contains"], "rank": rank})
        if rank is None or rank > item.get("top_k", 5):
            failures.append(f"expected {item['contains']!r} in {item['path']} top {item.get('top_k', 5)}, got rank {rank}")

    return failures, ranking


def rank_contains(values: Any, needle: str) -> int | None:
    if not isinstance(values, list):
        return None
    for index, value in enumerate(values, start=1):
        compact = json.dumps(value, sort_keys=True, separators=(",", ":"))
        if needle in compact:
            return index
    return None


def ndcg_at_5(rank: int | None) -> float:
    if rank is None or rank > 5:
        return 0.0
    import math
    return 1 / math.log2(rank + 1)


def get_path(value: Any, path: str) -> Any:
    current = value
    for part in path.split("."):
        if isinstance(current, dict):
            current = current.get(part)
        elif isinstance(current, list) and part.isdigit():
            current = current[int(part)]
        else:
            return None
    return current


if __name__ == "__main__":
    raise SystemExit(main())
