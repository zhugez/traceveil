import argparse
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description="Reproduce the synthetic offline Traceveil demo.")
parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/release/traceveil")
args = parser.parse_args()
binary = str(args.binary.resolve())
source = str(ROOT / "benchmarks/fixtures/demo.pi.jsonl")

def run(command):
    result = subprocess.run([binary, *command], check=True, capture_output=True, text=True)
    assert not result.stderr, result.stderr
    return result.stdout

print("Synthetic fixture, not a captured real agent run.")
print("$ traceveil diagnose benchmarks/fixtures/demo.pi.jsonl --agent pi --compact")
print(run(["diagnose", source, "--agent", "pi", "--compact"]))
report = json.loads(run(["diagnose", source, "--agent", "pi", "--json"]))
reference = report["data"]["findings"][0]["reference"]
print(f"$ traceveil inspect '{reference}' benchmarks/fixtures/demo.pi.jsonl --agent pi --json")
print(run(["inspect", reference, source, "--agent", "pi", "--json"]))
