import hashlib
import json
import os
import pathlib
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
dist = ROOT / "target/dist"
for line in (dist / "SHA256SUMS").read_text().splitlines():
    expected, name = line.split()
    assert pathlib.Path(name).name == name
    assert hashlib.sha256((dist / name).read_bytes()).hexdigest() == expected
results = {}
with tempfile.TemporaryDirectory(prefix="traceveil-install-") as directory:
    directory = pathlib.Path(directory)
    source = directory / "events.jsonl"
    source.write_bytes((ROOT / "benchmarks/fixtures/demo.jsonl").read_bytes())

    def verify(binary):
        def run(arguments):
            result = subprocess.run([str(binary), *arguments], cwd=directory, env={"PATH": "/nonexistent"}, check=True, capture_output=True, text=True)
            assert not result.stderr
            return result.stdout
        installed_version = run(["--version"]).strip()
        assert installed_version == f"traceveil {version}"
        assert version in run(["skills", "get", "core"])
        report = json.loads(run(["diagnose", str(source), "--agent", "jsonl", "--json"]))
        assert [finding["rule_id"] for finding in report["data"]["findings"]] == ["RF001", "LOOP001", "READ001"]
        return dict(version=installed_version, runtime_path="/nonexistent", cwd="fresh temporary directory", embedded_skill="passed", three_rule_report="passed", binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())

    with tarfile.open(dist / f"traceveil-{version}-linux-x86_64.tar.gz") as archive:
        binary = directory / "traceveil"
        binary.write_bytes(archive.extractfile(f"traceveil-{version}/traceveil").read())
        binary.chmod(0o700)
        assert archive.extractfile(f"traceveil-{version}/README.md").read() == (ROOT / "skill-data/core.md").read_bytes()
        for name in ("threat-model.md", "privacy-model.md"):
            assert archive.getmember(f"traceveil-{version}/docs/security/{name}").isfile()
    results["native_archive"] = verify(binary)
    prefix = directory / "npm"
    subprocess.run(["npm", "install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--prefix", str(prefix), str(dist / f"traceveil-{version}.tgz")], check=True, capture_output=True)
    results["npm_tarball"] = verify(prefix / "node_modules/.bin/traceveil")
    prefix = directory / "cargo"
    subprocess.run(["cargo", "install", "--path", str(ROOT), "--locked", "--offline", "--root", str(prefix), "--target-dir", str(ROOT / "target")], check=True, capture_output=True)
    results["cargo_clean_prefix"] = verify(prefix / "bin/traceveil")
results["artifact_checksums"] = "passed"
results["limits"] = "Linux x86_64 WSL2/glibc only. Build caches retained; installation prefixes, runtime PATH and runtime working directories were fresh."
(ROOT / "benchmarks/results/installation.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results))
