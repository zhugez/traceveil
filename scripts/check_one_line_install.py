import argparse
import hashlib
import json
import os
import pathlib
import shlex
import subprocess
import tarfile
import tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
URL="https://github.com/zhugez/traceveil/raw/main/install.sh"
parser=argparse.ArgumentParser(description="Exercise the one-line installer with isolated homes and stubbed HTTPS downloads")
parser.add_argument("--artifacts",type=pathlib.Path,default=ROOT/"target/dist")
parser.add_argument("--installer",type=pathlib.Path,default=ROOT/"target/release-prep/install.sh")
args=parser.parse_args()
manifest=json.loads((args.artifacts/"manifest.json").read_bytes())
assert manifest["platform"]=="linux-x86_64"
native=next(a for a in manifest["artifacts"] if a["kind"]=="native")
archive=(args.artifacts/native["name"]).resolve()
assert hashlib.sha256(archive.read_bytes()).hexdigest()==native["sha256"]
installer=args.installer.resolve()
script=installer.read_bytes()
assert script.endswith(b'main "$@"\n')
subprocess.run(["sh","-n",str(installer)],check=True)
with tarfile.open(archive) as bundle:
    assert all(not item.name.endswith("/install.sh") for item in bundle.getmembers())
    expected_binary=bundle.extractfile(f"traceveil-{manifest['version']}/traceveil").read()
with tempfile.TemporaryDirectory(prefix="traceveil-one-line-") as name:
    temporary=pathlib.Path(name).resolve()
    home=temporary/"home with spaces"
    home.mkdir()
    for profile in [".profile",".bashrc",".zshrc"]:(home/profile).write_text("preserve this profile\n")
    staging=temporary/"staging"
    staging.mkdir()
    (staging/"KEEP").write_text("unrelated")
    wrappers=temporary/"tools"
    wrappers.mkdir()
    curl=wrappers/"curl"
    curl.write_text('''#!/bin/sh
out=
url=
while [ "$#" -gt 0 ]; do
  case "$1" in -o|--output) out=$2; shift 2;; https://*) url=$1; shift;; *) shift;; esac
done
printf '%s\n' "$url" >> "$MOCK_REQUESTS"
case "$url" in
  https://github.com/zhugez/traceveil/raw/main/install.sh) cat "$MOCK_INSTALLER";;
  "$MOCK_ARCHIVE_URL") [ -n "$out" ] || exit 22; cp "$MOCK_ARCHIVE" "$out";;
  *) exit 22;;
esac
''')
    curl.chmod(0o700)
    requests=temporary/"requests"
    env=dict(os.environ,HOME=str(home),TRACEVEIL_HOME=str(home/".local"),TMPDIR=str(staging),PATH=str(wrappers)+os.pathsep+os.environ["PATH"],MOCK_REQUESTS=str(requests),MOCK_INSTALLER=str(installer),MOCK_ARCHIVE=str(archive),MOCK_ARCHIVE_URL=f"https://github.com/zhugez/traceveil/releases/download/v{manifest['version']}/{native['name']}")
    def pipe(options=(),environment=None,expected=0):
        command=f"curl -fsSL {URL} | sh"+(" -s -- "+shlex.join(options) if options else "")
        result=subprocess.run(["sh","-c",command],cwd=temporary,env=environment or env,capture_output=True)
        assert result.returncode==expected,(result.returncode,result.stderr)
        assert sorted(p.name for p in staging.iterdir())==["KEEP"]
        for profile in [".profile",".bashrc",".zshrc"]:assert (home/profile).read_text()=="preserve this profile\n"
        return result
    pipe()
    binary=home/".local/bin/traceveil"
    assert binary.read_bytes()==expected_binary
    assert requests.read_text().splitlines()==[URL,env["MOCK_ARCHIVE_URL"]]
    demo=subprocess.run([str(binary),"demo","--json"],cwd=temporary,env={"HOME":str(home),"PATH":"/nonexistent"},capture_output=True)
    assert demo.returncode==0 and json.loads(demo.stdout)["data"]["synthetic"] is True
    pipe(expected=2)
    assert binary.read_bytes()==expected_binary
    pipe(["--force"])
    corrupt=temporary/"corrupt.tar.gz"
    corrupt.write_bytes(b"corrupted archive")
    bad=dict(env,MOCK_ARCHIVE=str(corrupt))
    pipe(["--force"],bad,2)
    assert binary.read_bytes()==expected_binary
    assert not (temporary/"checksum-prefix").exists()
    pipe(["--prefix",str(temporary/"checksum-prefix")],bad,2)
    assert not (temporary/"checksum-prefix").exists()
    overrides=temporary/"overrides"
    overrides.mkdir()
    getconf=overrides/"getconf"
    getconf.write_text("#!/bin/sh\nprintf 'glibc 2.9\\n'\n")
    getconf.chmod(0o700)
    old=dict(env,PATH=str(overrides)+os.pathsep+env["PATH"])
    requests.write_text("")
    pipe(["--prefix",str(temporary/"old-libc")],old,2)
    assert requests.read_text().splitlines()==[URL] and not (temporary/"old-libc").exists()
    uname=overrides/"uname"
    uname.write_text('#!/bin/sh\ncase "$1" in -s) echo Darwin;; -m) echo arm64;; esac\n')
    uname.chmod(0o700)
    requests.write_text("")
    pipe(["--prefix",str(temporary/"unsupported")],old,2)
    assert requests.read_text().splitlines()==[URL] and not (temporary/"unsupported").exists()
    truncated=temporary/"truncated.sh"
    truncated.write_bytes(script[:script.index(b'mv -f "$binary_tmp" "$binary"')])
    pipe(["--prefix",str(temporary/"truncated-prefix")],dict(env,MOCK_INSTALLER=str(truncated)),2)
    assert not (temporary/"truncated-prefix").exists() and binary.read_bytes()==expected_binary
    empty=temporary/"empty.sh"
    empty.write_bytes(b"")
    pipe(["--prefix",str(temporary/"empty-prefix")],dict(env,MOCK_INSTALLER=str(empty)))
    assert not (temporary/"empty-prefix").exists()
    pipe(["--uninstall"])
    assert not binary.exists()
    invalid_artifacts=temporary/"invalid-artifacts"
    invalid_artifacts.mkdir()
    invalid=dict(manifest,version="0.2.0'; touch INJECTED; #")
    (invalid_artifacts/"manifest.json").write_text(json.dumps(invalid))
    rejected=subprocess.run(["python3",str(ROOT/"scripts/prepare_release.py"),"--artifacts",str(invalid_artifacts),"--output",str(temporary/"invalid-version")],capture_output=True)
    assert rejected.returncode==2 and not (temporary/"invalid-version/install.sh").exists()
    for item in manifest["artifacts"]:
        (invalid_artifacts/item["name"]).write_bytes((args.artifacts/item["name"]).read_bytes())
    invalid=dict(manifest,minimum_glibc="2.34; touch INJECTED")
    (invalid_artifacts/"manifest.json").write_text(json.dumps(invalid))
    rejected=subprocess.run(["python3",str(ROOT/"scripts/prepare_release.py"),"--artifacts",str(invalid_artifacts),"--output",str(temporary/"invalid-libc")],capture_output=True)
    assert rejected.returncode!=0 and b"measured glibc requirement" in rejected.stderr
    assert not (temporary/"invalid-libc/install.sh").exists()
print(json.dumps(dict(one_line_command=f"curl -fsSL {URL} | sh",transport="Stubbed HTTPS only; public endpoint and release were not published or fetched",pinned_version=manifest["version"],pinned_archive_sha256=native["sha256"],minimum_glibc=manifest["minimum_glibc"],binary_sha256=hashlib.sha256(expected_binary).hexdigest(),checks=["default pipe install without version/hash flags","fresh-home standalone demo with PATH=/nonexistent","existing install preserved without force","explicit force update","checksum failure preserves existing install","checksum failure leaves no new installation","older glibc rejected before archive download","unsupported platform rejected before archive download","truncated script leaves no installation","empty pipeline does not install and may exit zero","uninstall","profiles and unrelated staging file preserved","manifest version injection rejected","manifest glibc injection rejected"],publication=False)))
