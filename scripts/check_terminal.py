import argparse
import fcntl
import json
import os
import pathlib
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

parser = argparse.ArgumentParser(description="Exercise real terminal navigation, cancellation and restoration.")
parser.add_argument("binary", type=pathlib.Path)
args = parser.parse_args()
binary = str(args.binary.resolve())
results = []

def case(name, keys, expected, arguments=(), marker=b"What would you like", followup=None):
    with tempfile.TemporaryDirectory(prefix="traceveil-terminal-") as temp:
        root = pathlib.Path(temp).resolve()
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 40, 0, 0))
        before = termios.tcgetattr(slave)
        def session():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)
        env = dict(os.environ, HOME=str(root), USERPROFILE=str(root), TRACEVEIL_CONFIG=str(root / "config.json"), NO_COLOR="1", TERM="xterm", COLUMNS="40")
        env.pop("PI_CODING_AGENT_DIR", None)
        env.pop("PI_CODING_AGENT_SESSION_DIR", None)
        child = subprocess.Popen([binary, *arguments], stdin=slave, stdout=slave, stderr=slave, cwd=root, env=env, preexec_fn=session)
        output = bytearray()
        sent = False
        followed = False
        deadline = time.monotonic() + 5
        try:
            while time.monotonic() < deadline:
                ready, _, _ = select.select([master], [], [], 0.05)
                if ready:
                    output.extend(os.read(master, 65536))
                if not sent and marker in output:
                    if keys is None:
                        child.send_signal(signal.SIGINT)
                    else:
                        os.write(master, keys)
                    sent = True
                if sent and followup and not followed and followup[0] in output:
                    os.write(master, followup[1])
                    followed = True
                if child.poll() is not None:
                    while select.select([master], [], [], 0)[0]:
                        output.extend(os.read(master, 65536))
                    break
            if child.poll() is None:
                child.kill()
                child.wait()
                raise AssertionError((name, "terminal interaction timed out"))
            status = child.returncode if child.returncode >= 0 else 128 - child.returncode
            restored = termios.tcgetattr(slave) == before
            hidden = output.rfind(b"\x1b[?25l")
            cursor = hidden < 0 or output.rfind(b"\x1b[?25h") > hidden
            row = dict(case=name, exit=status, terminal_restored=restored, cursor_visible=cursor, synthetic_report=b"SYNTHETIC DEMO" in output)
            results.append(row)
            assert status == expected and restored and cursor, row
            assert followup is None or followed, row
            if name.endswith("demo"):
                assert row["synthetic_report"], row
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            os.close(master)
            os.close(slave)

case("escape", b"\x1b", 0)
case("keyboard-demo", b"jj\r", 0)
case("arrow-demo", b"\x1b[B\x1b[B\r", 0)
case("ctrl-c", b"\x03", 130)
case("external-sigint", None, 130)
case("confirm-ctrl-c", b"\x03", 130, ["analyze"], b"Read local session headers")
case("input-ctrl-c", b"\x03", 130, ["init"], b"Authorized Pi session directory")
case("confirm-external-sigint", None, 130, ["analyze"], b"Read local session headers")
case("input-external-sigint", None, 130, ["init"], b"Authorized Pi session directory")
case("consecutive-prompts-success", b"jjj\r", 0, followup=(b"Authorized Pi session directory", b".\r"))
case("consecutive-prompts-cancel", b"jjj\r", 130, followup=(b"Authorized Pi session directory", b"\x03"))
print(json.dumps({"terminal_checks": results}))
