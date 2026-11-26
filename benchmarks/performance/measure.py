import argparse
import hashlib
import json
import os
import pathlib
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description="Measure the offline CLI on synthetic inputs.")
parser.add_argument("--binary", type=pathlib.Path, default=ROOT / "target/release/traceveil")
parser.add_argument("--events", type=int, default=20000)
parser.add_argument("--runs", type=int, default=5)
parser.add_argument("--output", type=pathlib.Path, default=ROOT / "benchmarks/results/performance.json")
args = parser.parse_args()
if not 1 <= args.events <= 40000 or not 1 <= args.runs <= 20:
    parser.error("Use 1–40000 operations and 1–20 runs.")
binary = str(args.binary.resolve())
results = {}
with tempfile.TemporaryDirectory(prefix="traceveil-perf-") as directory:
    directory = pathlib.Path(directory)
    canonical = directory / "canonical.jsonl"
    pi = directory / "pi.jsonl"
    with canonical.open("w") as c, pi.open("w") as p:
        p.write(json.dumps(dict(type="session", version=3, id="performance")) + "\n")
        parent = None
        for i in range(args.events):
            data = dict(tool="bash", arguments=dict(command="synthetic-failure"), output="controlled failure", success=False)
            c.write(json.dumps(dict(schema_version="1.0", event_id=f"e{i}", session_id="performance", event_type="tool.execution.completed", data=data)) + "\n")
            p.write(json.dumps(dict(type="message", id=f"a{i}", parentId=parent, message=dict(role="assistant", stopReason="toolUse", content=[dict(type="toolCall", id=f"call{i}", name="bash", arguments=data["arguments"])]))) + "\n")
            p.write(json.dumps(dict(type="message", id=f"r{i}", parentId=f"a{i}", message=dict(role="toolResult", toolCallId=f"call{i}", toolName="bash", isError=True, content=[dict(type="text", text=data["output"])]))) + "\n")
            parent = f"r{i}"
    for adapter, source in (("jsonl", canonical), ("pi", pi)):
        measurements = []
        for index in range(args.runs):
            rss_file = directory / "rss"
            command = [binary, "diagnose", str(source), "--agent", adapter, "--json"]
            if platform.system() == "Linux":
                command = ["/usr/bin/time", "-f", "%M", "-o", str(rss_file), *command]
            started = time.perf_counter()
            result = subprocess.run(command, check=True, capture_output=True)
            elapsed = time.perf_counter() - started
            assert not result.stderr, result.stderr
            report = json.loads(result.stdout)
            assert report["ok"] and report["data"]["coverage"]["tool_results"] == args.events
            measurements.append(dict(wall_ms=elapsed * 1000, peak_rss_kib=int(rss_file.read_text()) if rss_file.exists() else None, output_bytes=len(result.stdout)))
        median_ms = statistics.median(row["wall_ms"] for row in measurements)
        results[adapter] = dict(input_bytes=source.stat().st_size, operations=args.events, measurements=measurements, median_wall_ms=median_ms, input_mib_per_second=source.stat().st_size / 1048576 / (median_ms / 1000))
    startup = []
    for _ in range(args.runs):
        started = time.perf_counter()
        subprocess.run([binary, "doctor", "--json"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        startup.append((time.perf_counter() - started) * 1000)
source_hash = hashlib.sha256()
for source in sorted([*ROOT.glob("src/*.rs"), ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "skill-data/core.md"]):
    source_hash.update(str(source.relative_to(ROOT)).encode() + b"\0" + source.read_bytes())
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
cpu = next((line.split(":", 1)[1].strip() for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), None) if platform.system() == "Linux" else platform.processor()
output = dict(version=subprocess.check_output([binary, "--version"], text=True).strip(), git_commit=revision, uncommitted_source=True, source_tree_sha256=source_hash.hexdigest(), binary_sha256=hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest(), binary_bytes=pathlib.Path(binary).stat().st_size, environment=dict(platform=platform.platform(), cpu=cpu, logical_cpus=os.cpu_count(), physical_memory_bytes=os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") if hasattr(os, "sysconf") else None, rustc=subprocess.check_output(["rustc", "--version"], text=True).strip()), startup_doctor_wall_ms=startup, workloads=results, limitations="Single local synthetic workload. OS cache is uncontrolled; these are not verified cold-start measurements. Diagnose wall times include the GNU time wrapper on Linux. No competitor or end-to-end agent-effectiveness comparison.")
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(output, indent=2) + "\n")
print(json.dumps(output))
