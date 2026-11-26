import argparse
import hashlib
import json
import pathlib
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description="Evaluate case-level candidate-pattern labels, not root-cause accuracy.")
parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/release/traceveil")
parser.add_argument("--output", type=pathlib.Path, default=ROOT / "benchmarks/results/diagnostic.json")
args = parser.parse_args()
labels = json.loads((ROOT / "benchmarks/fixtures/labels.json").read_text())
rules = ("RF001", "LOOP001", "READ001")
counts = {split: {rule: dict(tp=0, fp=0, fn=0, tn=0) for rule in rules} for split in ("dev", "heldout")}
cases = []
for case in labels["cases"]:
    source = ROOT / "benchmarks/fixtures" / case["file"]
    started = time.perf_counter()
    result = subprocess.run([str(args.binary.resolve()), "diagnose", str(source), "--agent", "jsonl", "--json"], check=True, capture_output=True)
    elapsed = (time.perf_counter() - started) * 1000
    assert not result.stderr
    report = json.loads(result.stdout)["data"]
    records = [json.loads(line) for line in source.read_text().splitlines()]
    for finding in report["findings"]:
        for evidence in finding["evidence"]:
            record = records[evidence["record_line"] - 1]
            assert hashlib.sha256(record["event_id"].encode()).hexdigest() == evidence["source_event_sha256"]
            encoded = json.dumps(record["data"]["arguments"], sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
            assert hashlib.sha256(encoded).hexdigest() == evidence["arguments_sha256"]
            assert hashlib.sha256(record["data"]["output"].encode()).hexdigest() == evidence["output_sha256"]
    expected = set(case["expected_rules"])
    actual = {finding["rule_id"] for finding in report["findings"]}
    for rule in rules:
        label = "tp" if rule in expected and rule in actual else "fp" if rule in actual else "fn" if rule in expected else "tn"
        counts[case["split"]][rule][label] += 1
    cases.append({"file": case["file"], "split": case["split"], "expected": sorted(expected), "observed": sorted(actual), "cli_wall_ms": elapsed, "evidence_fingerprints": "verified", "source_sha256": report["source_sha256"]})
for split in counts.values():
    for row in split.values():
        tp, fp, fn, tn = (row[key] for key in ("tp", "fp", "fn", "tn"))
        row.update(precision=tp / (tp + fp) if tp + fp else None, recall=tp / (tp + fn) if tp + fn else None, false_positive_rate=fp / (fp + tn) if fp + tn else None, false_negative_rate=fn / (fn + tp) if fn + tp else None)
output = {"unit": "case-level presence/absence per rule", "limitations": labels["methodology"], "counts": counts, "cases": cases}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(output, indent=2) + "\n")
print(json.dumps({"cases": len(cases), "counts": counts, "output": str(args.output)}))
if any(row["fp"] or row["fn"] for split in counts.values() for row in split.values()):
    raise SystemExit(1)
