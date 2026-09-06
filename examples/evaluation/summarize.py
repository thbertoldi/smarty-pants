"""Recompute diagnostic counts and timings; does not score semantic quality."""

import argparse
import hashlib
import json
import statistics
from pathlib import Path


def read_jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def summarize(rows):
    times = [r["ms"] for r in rows if "error" not in r]
    return {
        "cases": len(rows),
        "generation_errors": [r["id"] for r in rows if "error" in r],
        "latency_ms": {
            "median": round(statistics.median(times), 3) if times else None,
            "min": min(times) if times else None,
            "max": max(times) if times else None,
        },
        "maximum_observed_peak_rss_kib": max(r.get("peak_rss_kib") or 0 for r in rows),
        "literal_guard_blocks": [r["id"] for r in rows if r.get("fidelity_error")],
        "language_detector_mismatches": [r["id"] for r in rows
            if "error" not in r and r["detected_language"] != r["expected_language"]],
        "fixture_literal_misses": {r["id"]: r["missing"] for r in rows if r.get("missing")},
        "fixture_forbidden_phrases": {r["id"]: r["unexpected"] for r in rows if r.get("unexpected")},
        "cases_with_unknown_input_tokens": (
            [r["id"] for r in rows if r["unknown_input_tokens"] > 0]
            if all("unknown_input_tokens" in r for r in rows) else None
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--cases", type=Path, default=Path(__file__).with_name("comparison.jsonl"))
    args = parser.parse_args()
    cases = read_jsonl(args.cases)
    by_id = {case["id"]: case for case in cases}
    if not cases or len(by_id) != len(cases):
        parser.error("empty corpus or duplicate case IDs")
    result = {"corpus_sha256": hashlib.sha256(args.cases.read_bytes()).hexdigest(), "models": {}}
    for model in ("qwen", "coedit"):
        rows = read_jsonl(args.directory / (model + ".jsonl"))
        if len(rows) != len(cases) or {r["id"] for r in rows} != set(by_id):
            parser.error(f"missing/duplicate/unknown IDs in {model} results")
        for row in rows:
            case = by_id[row["id"]]
            if row["input"] != case["text"] or row["expected_language"] != case["language"]:
                parser.error(f"source/language mismatch for {row['id']}")
            if row["first_request_includes_load"]:
                parser.error("this comparison requires an excluded warmup")
        result["models"][model] = {"all": summarize(rows), "by_language": {
            language: summarize([r for r in rows if r["expected_language"] == language])
            for language in sorted({case["language"] for case in cases})
        }}
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
