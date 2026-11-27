import hashlib
import json
import pathlib
import os
import subprocess
import sys
import tempfile

import jsonschema

ROOT = pathlib.Path(__file__).resolve().parents[1]
binary = str(pathlib.Path(sys.argv[1]).resolve()) if len(sys.argv) > 1 else str(ROOT / "target/release/traceveil")
event_schema = json.loads((ROOT / "schemas/event.schema.json").read_text())
report_schema = json.loads((ROOT / "schemas/report.schema.json").read_text())
jsonschema.Draft202012Validator.check_schema(event_schema)
jsonschema.Draft202012Validator.check_schema(report_schema)

def run(arguments, source=None, status=0):
    env=dict(os.environ,TRACEVEIL_CONFIG=str(ROOT/"target/nonexistent-contract-config.json"))
    result = subprocess.run([binary, *arguments], input=source, capture_output=True,env=env)
    assert result.returncode == status, (arguments, result.returncode, result.stderr)
    assert not result.stderr, result.stderr
    return result.stdout

files = sorted((ROOT / "benchmarks/fixtures").rglob("*.jsonl"))
validated = 0
for source in files:
    if source.name.endswith(".pi.jsonl"):
        continue
    for line in source.read_text().splitlines():
        jsonschema.validate(json.loads(line), event_schema)
    response = json.loads(run(["diagnose", str(source), "--agent", "jsonl", "--json"]))
    jsonschema.validate(response, report_schema)
    validated += 1

pi = str(ROOT / "benchmarks/fixtures/demo.pi.jsonl")
response = json.loads(run(["diagnose", pi, "--agent", "pi", "--json"]))
jsonschema.validate(response, report_schema)
for command in [["analyze",pi,"--json"],["demo","--json"],["analyze","--demo","--rule-threshold","RF001=4","--suppress","READ001","--max-gap-seconds","321","--json"]]:
    current=json.loads(run(command))
    jsonschema.validate(current,report_schema)
    assert current["data"]["max_gap_seconds"] in [600,321]
reference = response["data"]["findings"][0]["reference"]
inspection = json.loads(run(["inspect", reference, pi, "--agent", "pi", "--json"]))
jsonschema.validate(inspection["data"]["finding"], {"$ref": "#/$defs/Finding", "$defs": report_schema["$defs"]})
assert inspection["data"]["finding"] == response["data"]["findings"][0]
error = json.loads(run(["diagnose", "--json"], status=2))
jsonschema.validate(error, report_schema)

skill = (ROOT / "skills/traceveil/SKILL.md").read_text()
assert skill.startswith("---\n")
frontmatter = dict(line.split(": ", 1) for line in skill.split("---", 2)[1].strip().splitlines())
assert frontmatter["name"] == "traceveil"
assert 1 <= len(frontmatter["description"]) <= 1024
assert len(frontmatter["compatibility"]) <= 500
runtime = run(["skills", "get", "core"]).decode()
assert runtime == (ROOT / "skill-data/core.md").read_text()
assert run(["--version"]).decode().strip().split()[-1] in runtime

examples = [
    ("traceveil --help", ["--help"]),
    ("traceveil diagnose --help", ["diagnose", "--help"]),
    ("traceveil diagnose SESSION --agent pi --compact", ["diagnose", pi, "--agent", "pi", "--compact"]),
    ("traceveil inspect REFERENCE SESSION --agent pi --json", ["inspect", reference, pi, "--agent", "pi", "--json"]),
    ("traceveil diagnose events.jsonl --agent jsonl --json", ["diagnose", str(ROOT / "benchmarks/fixtures/demo.jsonl"), "--agent", "jsonl", "--json"]),
]
for documented, actual in examples:
    assert documented in runtime, documented
    run(actual)
assert "traceveil diagnose --stdin --json" in runtime
run(["diagnose", "--stdin", "--json"], (ROOT / "benchmarks/fixtures/demo.jsonl").read_bytes())
with tempfile.TemporaryDirectory(prefix="traceveil-contract-") as directory:
    source = pathlib.Path(directory) / "session.jsonl"
    source.write_bytes(pathlib.Path(pi).read_bytes())
    before = hashlib.sha256(source.read_bytes()).hexdigest()
    assert "traceveil sessions --session-dir DIR --json" in runtime
    run(["sessions", "--session-dir", directory, "--json"])
    assert "traceveil diagnose --agent pi --last --session-dir DIR --compact" in runtime
    run(["diagnose", "--agent", "pi", "--last", "--session-dir", directory, "--compact"])
    assert hashlib.sha256(source.read_bytes()).hexdigest() == before
print(json.dumps({"canonical_fixtures_validated": validated, "pi_report_schema": "passed", "analyze_demo_policy_schema":"passed", "error_schema": "passed", "inspect_schema": "passed", "skill_frontmatter_and_runtime_examples": "passed"}))
