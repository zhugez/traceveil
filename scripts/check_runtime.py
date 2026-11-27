import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description="Check one controlled Linux runtime execution and synthetic privacy boundaries")
parser.add_argument("--binary",type=pathlib.Path,default=ROOT/"target/release/traceveil")
args=parser.parse_args()
binary=args.binary.resolve()
with tempfile.TemporaryDirectory(prefix="traceveil-runtime-") as name:
    work=pathlib.Path(name)
    config=work/"config.json"
    env=dict(os.environ,HOME=str(work),TRACEVEIL_CONFIG=str(config))
    source=work/"controlled.pi.jsonl"
    source.write_bytes((ROOT/"benchmarks/fixtures/demo.pi.jsonl").read_bytes())
    before=hashlib.sha256(source.read_bytes()).hexdigest()
    def invoke(arguments):
        result=subprocess.run([str(binary),*arguments],cwd=work,env=env,capture_output=True)
        assert result.returncode==0,(arguments,result.returncode,result.stderr)
        assert not result.stderr,result.stderr
        return json.loads(result.stdout)
    analyzed=invoke(["analyze",str(source),"--agent","pi","--json","--verbose"])
    config.write_text("invalid configuration")
    inspected=invoke(["inspect",analyzed["data"]["findings"][0]["reference"],str(source),"--agent","pi","--json"])
    assert inspected["data"]["finding"]==analyzed["data"]["findings"][0]
    config.unlink()
    markers=["TRACEVEIL_PRIVATE_ARGUMENT_MARKER","TRACEVEIL_PRIVATE_OUTPUT_MARKER","TRACEVEIL_PRIVATE_EVENT_MARKER","TRACEVEIL_PRIVATE_SESSION_MARKER"]
    records=[json.loads(line) for line in (ROOT/"benchmarks/fixtures/demo.jsonl").read_text().splitlines()]
    for index,record in enumerate(records):
        record["event_id"]=markers[2]+str(index)
        record["session_id"]=markers[3]
        if record["event_type"]=="tool.execution.completed":
            record["data"]["arguments"]["marker"]=markers[0]
            record["data"]["output"]=markers[1]+"\u001b[31m"
    sensitive=work/"controlled.jsonl"
    sensitive.write_text("\n".join(json.dumps(record) for record in records)+"\n")
    report=invoke(["analyze",str(sensitive),"--agent","jsonl","--json"])
    assert report["data"]["findings"]
    detail=invoke(["inspect",report["data"]["findings"][0]["reference"],str(sensitive),"--agent","jsonl","--json"])
    encoded=json.dumps([report,detail])
    assert not any(marker in encoded for marker in markers)
    trace=work/"syscalls.log"
    traced=subprocess.run(["strace","-f","-e","trace=process,network,openat,openat2","-o",str(trace),str(binary),"analyze",str(source),"--agent","pi","--json"],cwd=work,env=env,capture_output=True)
    assert traced.returncode==0 and not traced.stderr,traced.stderr
    lines=trace.read_text().splitlines()
    network_names=["socket","socketpair","bind","listen","accept","accept4","connect","getsockname","getpeername","shutdown","sendto","recvfrom","sendmsg","recvmsg","sendmmsg","recvmmsg","setsockopt","getsockopt"]
    execs=sum(bool(re.search(r"\bexecve(?:at)?\(",line)) for line in lines)
    children=sum(any(call+"(" in line for call in ["clone","clone3","fork","vfork"]) for line in lines)
    networking=sum(any(call+"(" in line for call in network_names) for line in lines)
    opens=[line for line in lines if "openat(" in line and "controlled.pi.jsonl" in line]
    flags=["O_RDONLY","O_NONBLOCK","O_NOFOLLOW","O_CLOEXEC"]
    assert execs==1 and children==networking==0
    assert len(opens)==1 and all(flag in opens[0] for flag in flags)
    assert hashlib.sha256(source.read_bytes()).hexdigest()==before
print(json.dumps(dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),workload="Controlled synthetic Pi v3 and canonical fixtures; no private session analyzed",json_verbose_stderr_empty=True,explicit_inspection_ignores_invalid_saved_policy=True,raw_arguments_outputs_and_identifiers_absent=True,exec_count=execs,additional_process_creations=children,network_syscalls=networking,network_syscall_names_checked=network_names,source_open_count=len(opens),source_open_flags_verified=flags,source_unchanged=True,scope="Only the captured execution and enumerated syscalls. Not a universal security proof.")))
