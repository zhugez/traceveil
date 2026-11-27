import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
dist=ROOT/"target/dist"
manifest_path=dist/"manifest.json"
original=hashlib.sha256(manifest_path.read_bytes()).hexdigest()
manifest=json.loads(manifest_path.read_bytes())
def run(arguments,env=None):
    return subprocess.run(arguments,cwd=ROOT,env=env,capture_output=True)
with tempfile.TemporaryDirectory(prefix="traceveil-signing-") as name:
    temporary=pathlib.Path(name)
    base=[sys.executable,str(ROOT/"scripts/prepare_release.py"),"--artifacts",str(dist)]
    plain=temporary/"unsigned"
    result=run([*base,"--output",str(plain)])
    assert result.returncode==0,result.stderr
    summary=json.loads(result.stdout)
    statement=json.loads((plain/"provenance.json").read_bytes())
    assert statement["self_reported"] is True and summary["signature"] is None
    assert statement["subjects"]==manifest["artifacts"] and statement["build"]==manifest["build"]
    assert not (plain/"provenance.json.asc").exists()
    if manifest["platform"]=="linux-x86_64":
        one_line=run([sys.executable,str(ROOT/"scripts/check_one_line_install.py"),"--artifacts",str(dist),"--installer",str(plain/"install.sh")])
        assert one_line.returncode==0,one_line.stderr
        result=run(["ruby","-c",str(plain/"traceveil.rb")])
        assert result.returncode==0,result.stderr
    skipped=temporary/"skipped"
    result=run([sys.executable,str(ROOT/"scripts/package.py"),"--skip-build","--out-dir",str(skipped)])
    assert result.returncode==0,result.stderr
    skipped_manifest=json.loads(result.stdout)
    assert skipped_manifest["build"]["command"] is None and skipped_manifest["build"]["source_binding"].startswith("Unverified existing binary")
    invalid=run([*base,"--output",str(temporary/"invalid"),"--sign-key","short"])
    assert invalid.returncode==2
    corrupted=temporary/"corrupt"
    corrupted.mkdir()
    shutil.copy2(manifest_path,corrupted/"manifest.json")
    for item in manifest["artifacts"]:
        shutil.copy2(dist/item["name"],corrupted/item["name"])
    with (corrupted/manifest["artifacts"][0]["name"]).open("ab") as output:output.write(b"corrupt")
    invalid=run([sys.executable,str(ROOT/"scripts/prepare_release.py"),"--artifacts",str(corrupted),"--output",str(temporary/"refused")])
    assert invalid.returncode!=0
    assert shutil.which("gpg") and shutil.which("gpgconf"),"GPG is needed for the isolated signing test"
    home=temporary/"gnupg"
    home.mkdir(mode=0o700)
    env=dict(os.environ,GNUPGHOME=str(home))
    try:
        generated=run(["gpg","--batch","--pinentry-mode","loopback","--passphrase","","--quick-generate-key","Traceveil signing test <test@traceveil.invalid>","ed25519","sign","0"],env)
        assert generated.returncode==0,generated.stderr
        listing=run(["gpg","--batch","--with-colons","--list-secret-keys"],env)
        assert listing.returncode==0,listing.stderr
        fingerprint=next(line.split(":")[9] for line in listing.stdout.decode().splitlines() if line.startswith("fpr:"))
        signed=temporary/"signed"
        prepared=run([*base,"--output",str(signed),"--sign-key",fingerprint],env)
        assert prepared.returncode==0,prepared.stderr
        evidence=signed/"provenance.json"
        signature=signed/"provenance.json.asc"
        verified=run(["gpg","--batch","--no-auto-key-retrieve","--verify",str(signature),str(evidence)],env)
        assert verified.returncode==0,verified.stderr
        evidence.write_bytes(evidence.read_bytes()+b"tampered")
        rejected=run(["gpg","--batch","--no-auto-key-retrieve","--verify",str(signature),str(evidence)],env)
        assert rejected.returncode!=0
        stale=run([*base,"--output",str(signed)],env)
        assert stale.returncode!=0
    finally:
        stopped=run(["gpgconf","--homedir",str(home),"--kill","gpg-agent"],env)
        assert stopped.returncode==0,stopped.stderr
assert hashlib.sha256(manifest_path.read_bytes()).hexdigest()==original
for item in manifest["artifacts"]:assert hashlib.sha256((dist/item["name"]).read_bytes()).hexdigest()==item["sha256"]
print(json.dumps(dict(unsigned_default="passed",pinned_one_line_installer="passed" if manifest["platform"]=="linux-x86_64" else "not_applicable",skipped_build_explicitly_unverified="passed",artifact_tamper_refusal="passed",explicit_full_fingerprint="passed",isolated_signature_verification="passed",signature_tamper_rejection="passed",stale_signature_refusal="passed",original_artifacts_unchanged=True,publication=False,signing_key="Ephemeral test key deleted; no maintainer key used")))
