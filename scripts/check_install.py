import argparse
import hashlib
import json
import os
import pathlib
import shutil
import shlex
import subprocess
import tarfile
import tempfile
import time

ROOT=pathlib.Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description="Verify locally prepared installation paths in isolated prefixes and homes.")
parser.add_argument("--skip-cargo",action="store_true")
parser.add_argument("--output",type=pathlib.Path,default=ROOT/"benchmarks/results/productization-installation.json")
args=parser.parse_args()
dist=ROOT/"target/dist"
manifest=json.loads((dist/"manifest.json").read_text())
assert manifest["platform"]=="linux-x86_64", "This acceptance driver currently runs on Linux x86_64; use native CI for other systems."
version=manifest["version"]
for artifact in manifest["artifacts"]:
    assert pathlib.Path(artifact["name"]).name==artifact["name"]
    assert hashlib.sha256((dist/artifact["name"]).read_bytes()).hexdigest()==artifact["sha256"]
native=next(a for a in manifest["artifacts"] if a["kind"]=="native")
npm=next(a for a in manifest["artifacts"] if a["kind"]=="private-local-npm")
results={"version":version,"platform":manifest["platform"],"runtime":"Fresh home/directory, PATH=/nonexistent; build caches retained.","publication":"No publication or remote download performed."}
with tempfile.TemporaryDirectory(prefix="traceveil-install-") as temp:
    directory=pathlib.Path(temp).resolve()
    work=directory/"project"
    work.mkdir()
    home=directory/"home"
    home.mkdir()
    sessions=directory/"sessions"
    sessions.mkdir()
    rows=[json.loads(l) for l in (ROOT/"benchmarks/fixtures/demo.pi.jsonl").read_text().splitlines()]
    rows[0]["cwd"]=str(work)
    source=sessions/"synthetic.jsonl"
    source.write_text("".join(json.dumps(r,separators=(",",":"))+"\n" for r in rows))
    source_digest=hashlib.sha256(source.read_bytes()).hexdigest()
    env={"PATH":"/nonexistent","HOME":str(home),"USERPROFILE":str(home),"TRACEVEIL_CONFIG":str(home/"config.json"),"NO_COLOR":"1"}
    def verify(binary):
        def run(arguments,status=0):
            result=subprocess.run([str(binary),*arguments],cwd=work,env=env,capture_output=True)
            assert result.returncode==status,(arguments,result.returncode,result.stderr)
            assert not result.stderr,(arguments,result.stderr)
            return result.stdout
        assert run(["--version"]).decode().strip()==f"traceveil {version}"
        assert version.encode() in run(["skills","get","core"])
        assert b"No session was analyzed" in run([])
        started=time.perf_counter()
        demo=json.loads(run(["demo","--json"]))
        elapsed=(time.perf_counter()-started)*1000
        assert demo["data"]["synthetic"] is True
        assert len(demo["data"]["findings"])==3
        assert json.loads(run(["inspect","f001","--demo","--json"]))["ok"]
        for command in ["doctor","agents","rules"]:
            assert json.loads(run([command,"--json"]))["ok"]
        listed=json.loads(run(["sessions","--agent","pi","--session-dir",str(sessions),"--json"]))
        assert listed["data"]["count"]==1
        report=json.loads(run(["analyze","--last","--session-dir",str(sessions),"--json"]))
        assert [f["rule_id"] for f in report["data"]["findings"]]==["RF001","LOOP001","READ001"]
        ref=report["data"]["findings"][0]["reference"]
        inspection=json.loads(run(["inspect",ref,str(source),"--json"]))
        assert inspection["data"]["finding"]==report["data"]["findings"][0]
        assert json.loads(run(["init","--session-dir",str(sessions),"--json"]))["data"]["configured"]
        assert json.loads(run(["analyze","--last","--json"]))["data"]==report["data"]
        assert json.loads(run(["skills","install","--agent","pi","--scope","project","--json"]))["data"]["installed"]
        assert json.loads(run(["skills","uninstall","--agent","pi","--scope","project","--json"]))["data"]["uninstalled"]
        assert b"traceveil" in run(["completions","bash"])
        assert hashlib.sha256(source.read_bytes()).hexdigest()==source_digest
        return dict(version=version,embedded_demo="passed",demo_wall_ms=elapsed,discovery_analysis_inspection="passed",json="passed",configuration="passed",skill_lifecycle="passed",source_unchanged=True,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
    unpacked=directory/"unpacked"
    unpacked.mkdir()
    with tarfile.open(dist/native["name"]) as archive:
        actual={}
        for entry in archive.getmembers():
            assert entry.isfile() and entry.uid==entry.gid==entry.mtime==0
            assert entry.name.startswith(f"traceveil-{version}/")
            relative=entry.name.removeprefix(f"traceveil-{version}/")
            assert ".." not in pathlib.PurePosixPath(relative).parts
            actual[relative]=hashlib.sha256(archive.extractfile(entry).read()).hexdigest()
        assert actual==manifest["native_files"]
        assert "docs/getting-started.md" in actual and "docs/examples/demo.txt" in actual
        results["native_archive_contents"]={"files_verified":len(actual),"entry_hashes":"matched manifest","onboarding_and_captured_demo":"included"}
        raw=archive.extractfile(f"traceveil-{version}/traceveil").read()
        binary=unpacked/"traceveil"
        binary.write_bytes(raw)
        binary.chmod(0o700)
        assert version.encode() in archive.extractfile(f"traceveil-{version}/share/traceveil/core.md").read()
    results["native_archive"]=verify(binary)
    installer=ROOT/"scripts/install.sh"
    prefix=directory/"prefix with spaces"
    install=["sh",str(installer),"--version",version,"--archive",str(dist/native["name"]),"--sha256",native["sha256"],"--prefix",str(prefix)]
    def shell(arguments,status=0,environment=None):
        result=subprocess.run(arguments,cwd=work,capture_output=True,env=environment)
        assert result.returncode==status,(arguments,result.returncode,result.stderr)
        return result
    shell(install)
    results["posix_installer"]=verify(prefix/"bin/traceveil")
    shell(install,2)
    shell([*install,"--force"])
    sentinel=prefix/"bin/.traceveil-install-new"
    sentinel.write_text("unrelated")
    shell([*install,"--force"])
    assert sentinel.read_text()=="unrelated"
    installed=prefix/"bin/traceveil"
    original=installed.read_bytes()
    installed.write_text("user modification")
    shell(["sh",str(installer),"--uninstall","--prefix",str(prefix)],2)
    assert installed.read_text()=="user modification"
    installed.write_bytes(original)
    shell(["sh",str(installer),"--uninstall","--prefix",str(prefix)])
    assert not installed.exists() and sentinel.exists() and (home/"config.json").exists()
    unowned=directory/"unowned"
    (unowned/"bin").mkdir(parents=True)
    (unowned/"bin/traceveil").write_text("preserve")
    bad=[*install]
    bad[-1]=str(unowned)
    shell([*bad,"--force"],2)
    assert (unowned/"bin/traceveil").read_text()=="preserve"
    corrupt=[*install]
    corrupt[corrupt.index("--sha256")+1]="0"*64
    corrupt[-1]=str(directory/"corrupt-prefix")
    shell(corrupt,2)
    assert not (directory/"corrupt-prefix").exists()
    mutable=directory/"mutable.tar.gz"
    shutil.copyfile(dist/native["name"],mutable)
    wrappers=directory/"wrappers"
    wrappers.mkdir()
    wrapper=wrappers/"tar"
    wrapper.write_text("#!/bin/sh\nstat -c %a \"$(dirname \"$2\")\" > \"$TRACEVEIL_RACE_MODES\"\nstat -c %a \"$2\" >> \"$TRACEVEIL_RACE_MODES\"\nprintf changed > \"$TRACEVEIL_RACE_SOURCE\"\nexec "+shlex.quote(shutil.which("tar"))+" \"$@\"\n")
    wrapper.chmod(0o700)
    race=[*install]
    race[race.index("--archive")+1]=str(mutable)
    race[-1]=str(directory/"race-prefix")
    shell(race,environment=dict(os.environ,PATH=str(wrappers)+os.pathsep+os.environ["PATH"],TRACEVEIL_RACE_SOURCE=str(mutable),TRACEVEIL_RACE_MODES=str(directory/"snapshot-modes")))
    assert (directory/"snapshot-modes").read_text().splitlines()==["700","600"]
    results["private_snapshot_permissions"]={"directory":"0700","archive":"0600"}
    assert mutable.read_bytes()!= (dist/native["name"]).read_bytes()
    assert (directory/"race-prefix/bin/traceveil").read_bytes()==raw
    dash=work/"-delete"
    shutil.copyfile(dist/native["name"],dash)
    keep=work/"KEEP"
    keep.write_text("preserve")
    dash_args=[*install]
    dash_args[dash_args.index("--archive")+1]="-delete"
    dash_args[-1]=str(directory/"dash-prefix")
    shell(dash_args)
    assert keep.read_text()=="preserve" and dash.exists()
    oversized=directory/"oversized.tar.gz"
    with oversized.open("wb") as output:output.truncate(104857601)
    oversize_args=[*install]
    oversize_args[oversize_args.index("--archive")+1]=str(oversized)
    oversize_args[-1]=str(directory/"oversized-prefix")
    shell(oversize_args,2)
    assert not (directory/"oversized-prefix").exists()
    results["archive_size_limit"]="Sparse archive over 100 MiB rejected before installation"
    curl_wrappers=directory/"curl-wrappers"
    curl_wrappers.mkdir()
    curl=curl_wrappers/"curl"
    curl.write_text('#!/bin/sh\nout=\nwhile [ "$#" -gt 0 ]; do case "$1" in -o|--output) out=$2; shift 2;; *) shift;; esac; done\nprintf partial > "$out"\nexit 28\n')
    curl.chmod(0o700)
    staging=directory/"download-staging"
    staging.mkdir()
    (staging/"KEEP").write_text("preserve")
    download=[*install]
    index=download.index("--archive")
    del download[index:index+2]
    download[-1]=str(directory/"download-prefix")
    shell(download,2,dict(os.environ,PATH=str(curl_wrappers)+os.pathsep+os.environ["PATH"],TMPDIR=str(staging)))
    assert sorted(p.name for p in staging.iterdir())==["KEEP"]
    assert not (directory/"download-prefix").exists()
    results["simulated_download_failure_cleanup"]="Partial curl output removed; unrelated staging file and destination preserved. Actual HTTPS release download remains untested."
    results["archive_filename_option_safety"]="Leading-dash archive installed; unrelated files preserved."
    results["verified_archive_snapshot"]="Original replaced at extraction; private verified snapshot installed unchanged."
    results["installer_refusal_paths"]={"checksum_mismatch":"passed","unowned_force_refusal":"passed","modified_uninstall_refusal":"passed","receipted_force_update":"passed","unrelated_staging_file_preserved":"passed","uninstall_preserves_config":"passed"}
    npmprefix=directory/"npm"
    subprocess.run(["npm","install","--offline","--ignore-scripts","--no-audit","--no-fund","--prefix",str(npmprefix),str(dist/npm["name"])],check=True,capture_output=True)
    results["npm_tarball"]=verify(npmprefix/"node_modules/.bin/traceveil")
    subprocess.run(["npm","uninstall","--offline","--ignore-scripts","--no-audit","--no-fund","--prefix",str(npmprefix),"traceveil"],check=True,capture_output=True)
    assert not (npmprefix/"node_modules/.bin/traceveil").exists()
    if not args.skip_cargo:
        cargoprefix=directory/"cargo"
        subprocess.run(["cargo","install","--path",str(ROOT),"--locked","--offline","--root",str(cargoprefix),"--target-dir",str(ROOT/"target")],check=True,capture_output=True)
        results["cargo_clean_prefix"]=verify(cargoprefix/"bin/traceveil")
        subprocess.run(["cargo","uninstall","--root",str(cargoprefix),"traceveil"],check=True,capture_output=True)
        assert not (cargoprefix/"bin/traceveil").exists()
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(results,indent=2)+"\n")
print(json.dumps(results))
