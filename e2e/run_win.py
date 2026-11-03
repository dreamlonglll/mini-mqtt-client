#!/usr/bin/env python3
"""Full end-to-end test of Mini MQTT Client on Windows (the counterpart of run.sh).

Starts a private mosquitto broker (TCP / WebSocket / TLS) and runs e2e/test_app.py
against the real application binary through driver_win.py. The application window
is moved to the monitor chosen by E2E_MONITOR (`largest` by default, `primary`, or
a 1-based index); input is posted to the window, so the mouse, the keyboard focus
and the other monitors stay free. The system clipboard is used.

Environment (as in run.sh): E2E_BIN (default target/debug/mini-mqtt-client.exe,
built if missing), E2E_WORK_DIR, E2E_ONLY, E2E_MONITOR, E2E_MOSQUITTO_DIR,
E2E_OPENSSL_DIR (default: the openssl that comes with Git for Windows).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def find_tool(name: str, *dirs) -> Path | None:
    for d in dirs:
        if d and (Path(d) / name).is_file():
            return Path(d) / name
    found = shutil.which(name)
    return Path(found) if found else None


def main() -> int:
    work = Path(os.environ.get("E2E_WORK_DIR") or ROOT / "target" / "e2e").resolve()
    binary = Path(os.environ.get("E2E_BIN") or ROOT / "target" / "debug" / "mini-mqtt-client.exe").resolve()
    tcp = os.environ.get("E2E_TCP_PORT", "18830")
    ws = os.environ.get("E2E_WS_PORT", "18083")
    tls = os.environ.get("E2E_TLS_PORT", "18883")

    mosquitto = find_tool(
        "mosquitto.exe",
        os.environ.get("E2E_MOSQUITTO_DIR"),
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "mosquitto",
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "mosquitto",
    )
    git = shutil.which("git")
    openssl = find_tool("openssl.exe", os.environ.get("E2E_OPENSSL_DIR"),
                        Path(git).resolve().parents[1] / "usr" / "bin" if git else None)
    missing = [n for n, p in (("mosquitto", mosquitto), ("openssl", openssl)) if p is None]
    if missing:
        print(f"missing tools: {' '.join(missing)} (winget install EclipseFoundation.Mosquitto; Git for Windows "
              "ships openssl; or set E2E_MOSQUITTO_DIR / E2E_OPENSSL_DIR)", file=sys.stderr)
        return 2
    try:
        import comtypes  # noqa: F401
        import PIL  # noqa: F401
        import yaml  # noqa: F401
    except ImportError as err:
        print(f"{err}: pip install comtypes pillow pyyaml", file=sys.stderr)
        return 2

    if not binary.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True)

    shutil.rmtree(work, ignore_errors=True)
    for sub in ("tls", "shots", "data"):
        (work / sub).mkdir(parents=True)

    # --- TLS material: a throwaway CA and a server certificate for localhost ---
    tls_dir = work / "tls"

    def ssl(*args: str):
        subprocess.run([str(openssl), *args], cwd=tls_dir, check=True, capture_output=True)

    ssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.crt", "-days", "2",
        "-subj", "/CN=E2E Test CA")
    ssl("req", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-out", "server.csr", "-subj", "/CN=localhost")
    (tls_dir / "ext.cnf").write_text("subjectAltName=DNS:localhost,IP:127.0.0.1\n")
    ssl("x509", "-req", "-in", "server.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial",
        "-out", "server.crt", "-days", "2", "-extfile", "ext.cnf")

    # --- Broker ------------------------------------------------------------------
    conf = work / "mosquitto.conf"
    conf.write_text(
        "per_listener_settings false\nallow_anonymous true\n"
        f"listener {tcp} 127.0.0.1\nprotocol mqtt\n"
        f"listener {ws} 127.0.0.1\nprotocol websockets\n"
        f"listener {tls} 127.0.0.1\nprotocol mqtt\n"
        f"cafile {(tls_dir / 'ca.crt').as_posix()}\n"
        f"certfile {(tls_dir / 'server.crt').as_posix()}\n"
        f"keyfile {(tls_dir / 'server.key').as_posix()}\n"
    )
    env = dict(
        os.environ,
        PATH=f"{mosquitto.parent}{os.pathsep}{os.environ.get('PATH', '')}",
        E2E_ROOT=str(ROOT), E2E_WORK=str(work), E2E_BIN=str(binary), E2E_DISPLAY="desktop",
        E2E_TCP_PORT=tcp, E2E_WS_PORT=ws, E2E_TLS_PORT=tls, E2E_CA=str(tls_dir / "ca.crt"),
        # The scenarios find controls by their (Chinese) accessible names; no update check, no
        # sliding dialogs either (see run.sh).
        MINI_MQTT_LANG="zh-CN", MINI_MQTT_NO_UPDATE_CHECK="1", MINI_MQTT_REDUCE_MOTION="1",
        PYTHONUTF8="1",
    )
    broker_log = open(work / "broker.log", "wb")
    broker = subprocess.Popen([str(mosquitto), "-c", str(conf), "-v"], stdout=broker_log, stderr=subprocess.STDOUT,
                              env=env, creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        pub = str(mosquitto.parent / "mosquitto_pub.exe")
        for _ in range(50):
            if subprocess.run([pub, "-h", "127.0.0.1", "-p", tcp, "-t", "e2e/ready", "-n"], capture_output=True).returncode == 0:
                break
            time.sleep(0.1)
        else:
            print(f"broker did not start, see {work / 'broker.log'}", file=sys.stderr)
            return 2
        status = subprocess.run([sys.executable, "-X", "utf8", str(ROOT / "e2e" / "test_app.py")], env=env).returncode
    finally:
        broker.terminate()
        broker.wait(5)
        broker_log.close()
    print(f"report: {work / 'report.md'}")
    return status


if __name__ == "__main__":
    sys.exit(main())
