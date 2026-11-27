import argparse
import json
import pathlib
import os
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description="Reproduce the synthetic offline Traceveil demo.")
parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/release/traceveil")
args = parser.parse_args()
binary = str(args.binary.resolve())

def run(command):
    result = subprocess.run([binary, *command], check=True, capture_output=True, text=True,env=dict(os.environ,NO_COLOR="1",COLUMNS="80"))
    assert not result.stderr, result.stderr
    return result.stdout

print("Synthetic fixture, not a captured real agent run.")
print("Captured with NO_COLOR=1 and COLUMNS=80.")
print("$ traceveil demo")
print(run(["demo"]))
report = json.loads(run(["demo", "--json"]))
assert report["data"]["synthetic"] is True
reference = report["data"]["findings"][0]["reference"]
print(f"$ traceveil inspect {reference} --demo --json")
print(run(["inspect", reference, "--demo", "--json"]))
