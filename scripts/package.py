import hashlib
import json
import pathlib
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
if platform.system() != "Linux" or platform.machine() != "x86_64" or platform.libc_ver()[0] != "glibc":
    raise SystemExit("Local packaging is currently verified only for Linux x86_64 glibc. Build the native Rust CLI directly on other targets.")
subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
binary = ROOT / "target/release/traceveil"
assert subprocess.check_output([str(binary), "--version"], text=True).strip() == f"traceveil {version}"
dist = ROOT / "target/dist"
dist.mkdir(parents=True, exist_ok=True)
archive = dist / f"traceveil-{version}-linux-x86_64.tar.gz"
with tarfile.open(archive, "w:gz") as tar:
    for source, name in ((binary, "traceveil"), (ROOT / "LICENSE", "LICENSE"), (ROOT / "skill-data/core.md", "README.md"), (ROOT / "SECURITY.md", "SECURITY.md")):
        tar.add(source, arcname=f"traceveil-{version}/{name}")
    tar.add(ROOT / "docs/security", arcname=f"traceveil-{version}/docs/security")
with tempfile.TemporaryDirectory(prefix="traceveil-package-", dir=dist) as stage:
    stage = pathlib.Path(stage)
    (stage / "bin").mkdir()
    shutil.copy2(binary, stage / "bin/traceveil")
    for name in ("LICENSE", "README.md", "SECURITY.md"):
        shutil.copy2(ROOT / name, stage / name)
    shutil.copy2(ROOT / "skill-data/core.md", stage / "README.md")
    shutil.copytree(ROOT / "docs/security", stage / "docs/security")
    shutil.copytree(ROOT / "skills", stage / "skills")
    package = dict(name="traceveil", version=version, private=True, description="A local-first diagnostic CLI for AI coding agents", license="GPL-3.0-only", bin={"traceveil": "bin/traceveil"}, os=["linux"], cpu=["x64"], libc=["glibc"], files=["bin", "skills", "docs/security", "README.md", "LICENSE", "SECURITY.md"], repository={"type": "git", "url": "https://github.com/zhugez/traceveil.git"})
    (stage / "package.json").write_text(json.dumps(package, indent=2) + "\n")
    packed = subprocess.run(["npm", "pack", str(stage), "--offline", "--ignore-scripts", "--json", "--pack-destination", str(dist)], check=True, capture_output=True, text=True)
    tarball = dist / json.loads(packed.stdout)[0]["filename"]
artifacts = [archive, tarball]
checksums = "".join(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in artifacts)
(dist / "SHA256SUMS").write_text(checksums)
print(json.dumps({"artifacts": [str(path) for path in artifacts], "checksums": str(dist / "SHA256SUMS"), "publication": "Not published. The local npm package is private and has no install scripts or network downloader."}))
