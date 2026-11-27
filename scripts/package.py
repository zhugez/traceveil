import argparse
import gzip
import hashlib
import io
import json
import pathlib
import os
import re
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description="Build deterministic native archives and a private local npm package. Never publishes.")
parser.add_argument("--skip-build", action="store_true")
parser.add_argument("--out-dir", type=pathlib.Path, default=ROOT / "target/dist")
args = parser.parse_args()
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(platform.system())
arch = {"x86_64": "x86_64", "AMD64": "x86_64", "aarch64": "arm64", "arm64": "arm64"}.get(platform.machine())
if not system or not arch:
    raise SystemExit("Unsupported packaging host. Build the Rust executable directly.")
if system=="linux" and platform.libc_ver()[0]!="glibc":
    raise SystemExit("Linux archive preparation currently requires glibc; build the Rust CLI directly on other libc implementations.")
def runtime_source():
    inputs=sorted([*ROOT.glob("src/*.rs"),ROOT/"Cargo.toml",ROOT/"Cargo.lock",ROOT/"skill-data/core.md",ROOT/"skills/traceveil/SKILL.md",ROOT/"benchmarks/fixtures/demo.jsonl"])
    entries={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
    return dict(files=entries,sha256=hashlib.sha256(json.dumps(entries,sort_keys=True,separators=(",",":")).encode()).hexdigest())
source_before=runtime_source()
name = "traceveil.exe" if system == "windows" else "traceveil"
binary = ROOT / "target/release" / name
build_command=None
compiler=None
if not args.skip_build:
    compiler=subprocess.check_output(["rustc","-vV"],cwd=ROOT,text=True)
    host=next(line.removeprefix("host: ") for line in compiler.splitlines() if line.startswith("host: "))
    assert host.startswith({"x86_64":"x86_64-","arm64":"aarch64-"}[arch]),"Rust compiler must match the packaging CPU"
    assert {"linux":"linux","macos":"darwin","windows":"windows"}[system] in host,"Rust compiler must match the packaging OS"
    build_command=["cargo","build","--release","--locked","--target",host,"--target-dir","target","--message-format=json-render-diagnostics"]
    compiled=subprocess.run(build_command,cwd=ROOT,check=True,capture_output=True,text=True)
    outputs=[json.loads(line) for line in compiled.stdout.splitlines()]
    executable=next(item["executable"] for item in reversed(outputs) if item.get("reason")=="compiler-artifact" and item.get("executable") and item.get("target",{}).get("name")=="traceveil" and pathlib.Path(item["manifest_path"]).resolve()==ROOT/"Cargo.toml")
    binary=pathlib.Path(executable)
assert subprocess.check_output([str(binary), "--version"], text=True).strip() == f"traceveil {version}"
dist = args.out_dir.resolve()
dist.mkdir(parents=True, exist_ok=True)
slug = f"{system}-{arch}"
files = {name: binary.read_bytes()}
for source in [ROOT / "README.md", ROOT / "LICENSE", ROOT / "SECURITY.md", ROOT / "CHANGELOG.md", *sorted(p for p in (ROOT / "docs").rglob("*") if p.suffix in [".md",".txt"]), *sorted((ROOT / "assets/brand").glob("*"))]:
    if source.is_symlink():
        raise SystemExit("Packaging does not follow source symlinks.")
    if source.is_file():
        files[source.relative_to(ROOT).as_posix()] = source.read_bytes()
files["share/traceveil/core.md"] = (ROOT / "skill-data/core.md").read_bytes()
files["share/traceveil/skills/traceveil/SKILL.md"] = (ROOT / "skills/traceveil/SKILL.md").read_bytes()
for shell in ["bash", "zsh", "fish", "powershell", "elvish"]:
    files[f"completions/traceveil.{shell}"] = subprocess.check_output([str(binary), "completions", shell])
archive = dist / f"traceveil-{version}-{slug}.{'zip' if system == 'windows' else 'tar.gz'}"
prefix = f"traceveil-{version}/"
if system == "windows":
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as target:
        for path, data in sorted(files.items()):
            info = zipfile.ZipInfo(prefix + path, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o755 if path == name else 0o644) << 16
            target.writestr(info, data)
else:
    with archive.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as zipped, tarfile.open(fileobj=zipped, mode="w") as target:
        for path, data in sorted(files.items()):
            info = tarfile.TarInfo(prefix + path)
            info.size = len(data)
            info.mode = 0o755 if path == name else 0o644
            target.addfile(info, io.BytesIO(data))
artifacts = [{"name":archive.name,"kind":"native","sha256":hashlib.sha256(archive.read_bytes()).hexdigest(),"bytes":archive.stat().st_size}]
if system == "linux" and arch == "x86_64":
    with tempfile.TemporaryDirectory(prefix="traceveil-npm-", dir=dist) as temp:
        stage = pathlib.Path(temp)
        (stage / "bin").mkdir()
        (stage/"bin"/name).write_bytes(files[name])
        (stage/"bin"/name).chmod(0o755)
        for path,data in files.items():
            if path in ["README.md","LICENSE","SECURITY.md","CHANGELOG.md"] or path.startswith(("assets/brand/","docs/")):
                output=stage/path
                output.parent.mkdir(parents=True,exist_ok=True)
                output.write_bytes(data)
        package = dict(name="traceveil", version=version, private=True, description="Debug your coding agents. Local diagnostics with evidence.", license="GPL-3.0-only", bin={"traceveil": f"bin/{name}"}, os=[{"macos": "darwin", "windows": "win32"}.get(system, system)], cpu=[{"x86_64": "x64", "arm64": "arm64"}[arch]], files=["bin", "README.md", "LICENSE", "SECURITY.md", "CHANGELOG.md", "assets", "docs"], repository={"type": "git", "url": "https://github.com/zhugez/traceveil.git"})
        if system == "linux":
            package["libc"] = ["glibc"]
            if platform.libc_ver()[0] != "glibc":
                raise SystemExit("Linux npm preparation requires a glibc host; no musl claim is made.")
        (stage / "package.json").write_text(json.dumps(package, indent=2) + "\n")
        result = subprocess.run(["npm", "pack", str(stage), "--offline", "--ignore-scripts", "--json", "--pack-destination", str(stage)], check=True, capture_output=True, text=True)
        packed = stage / json.loads(result.stdout)[0]["filename"]
        npm = dist / f"traceveil-{version}-{slug}.npm.tgz"
        shutil.copyfile(packed, npm)
    artifacts.append({"name":npm.name,"kind":"private-local-npm","sha256":hashlib.sha256(npm.read_bytes()).hexdigest(),"bytes":npm.stat().st_size})
minimum_glibc=None
if system=="linux":
    elf=subprocess.check_output(["readelf","--version-info","--wide",str(binary)],text=True,env=dict(os.environ,LC_ALL="C"))
    needs=elf.split("Version needs section",1)[1]
    versions=re.findall(r"Name: GLIBC_([0-9]+(?:\.[0-9]+)+)",needs)
    assert versions,"Cannot determine the native binary glibc requirement"
    minimum_glibc=max(versions,key=lambda version:tuple(map(int,version.split("."))))
source_after=runtime_source()
assert source_before==source_after,"Runtime inputs changed during build/packaging. No manifest was emitted."
build=dict(command=build_command,compiler=compiler,output=binary.relative_to(ROOT).as_posix(),source_before_sha256=source_before["sha256"],source_after_sha256=source_after["sha256"],inputs=source_before["files"],source_binding="Source unchanged across successful Cargo build and packaging" if not args.skip_build else "Unverified existing binary: build skipped; only version and artifact bytes checked",git_commit=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),dirty_runtime_inputs=bool(subprocess.check_output(["git","status","--porcelain","--untracked-files=all","--","src","Cargo.toml","Cargo.lock","skill-data","skills"],cwd=ROOT)))
manifest = dict(version=version,build=build, platform=slug, minimum_glibc=minimum_glibc, artifacts=artifacts, native_files={path:hashlib.sha256(data).hexdigest() for path,data in sorted(files.items())}, binary_sha256=hashlib.sha256(files[name]).hexdigest(), publication="Not published. Local npm preparation is Linux x86_64/glibc only, private, and contains no scripts or downloader.", provenance="Checksums and self-reported local build metadata; unsigned. No independent builder attestation is asserted.")
(dist / "SHA256SUMS").write_text("".join(f"{a['sha256']}  {a['name']}\n" for a in artifacts))
(dist / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest))
