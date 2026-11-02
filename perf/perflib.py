"""Shared helpers for the perf experiments (run inside e2e/run.sh via E2E_SCRIPT).

* QuietBroker  - replaces the verbose (-v) mosquitto of run.sh with a quiet one
                 on the same ports (per-message logging would distort numbers),
                 and can be killed / restarted for the resilience test.
* Sampler      - samples app CPU (total, main/UI thread, per thread group), RSS,
                 threads and fds, plus CPU of the broker / load generator /
                 AT-SPI helpers, into a JSON-lines file.
* LoadGen      - wrapper around perf/loadgen (Rust, rumqttc) `mqtt-load`.
* UI probes    - click latency on the "消息过滤" segmented control, the newest
                 message shown in the list (seq + send time => display lag),
                 the "消息数" counter.
"""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(os.environ["E2E_ROOT"])
sys.path.insert(0, str(ROOT / "e2e"))
from driver import App, E2EError, Query, load_yaml, wait_until  # noqa: E402,F401

import yaml  # noqa: E402

WORK = Path(os.environ["E2E_WORK"])
OUT = Path(os.environ.get("PERF_OUT", str(WORK / "results")))
OUT.mkdir(parents=True, exist_ok=True)
TCP = int(os.environ["E2E_TCP_PORT"])
DISPLAY = os.environ["E2E_DISPLAY"]
BIN = Path(os.environ["E2E_BIN"])
LOADGEN = os.environ.get("PERF_LOADGEN", str(ROOT / "perf/loadgen/target/release/mqtt-load"))
HZ = os.sysconf("SC_CLK_TCK")


def log(*args):
    print(time.strftime("%H:%M:%S"), *args, flush=True)


# ---------------------------------------------------------------------------
# Broker


class QuietBroker:
    def __init__(self):
        self.conf = WORK / "mosquitto.conf"
        self.proc: subprocess.Popen | None = None
        self.log_path = WORK / "broker-quiet.log"

    def ready(self) -> bool:
        r = subprocess.run(["mosquitto_pub", "-h", "127.0.0.1", "-p", str(TCP), "-t", "perf/ready", "-n"],
                           capture_output=True)
        return r.returncode == 0

    def takeover(self):
        subprocess.run(["pkill", "-f", f"mosquitto -c {self.conf} -v"], check=False)
        time.sleep(0.5)
        self.start()

    def start(self):
        log_f = open(self.log_path, "ab")
        for _ in range(50):
            self.proc = subprocess.Popen(["mosquitto", "-c", str(self.conf)], stdout=log_f, stderr=subprocess.STDOUT)
            time.sleep(0.3)
            if self.proc.poll() is None:
                break
            time.sleep(0.3)  # port still in TIME_WAIT / old broker exiting
        wait_until(self.ready, 10, desc="quiet broker ready")

    @property
    def pid(self):
        return self.proc.pid if self.proc and self.proc.poll() is None else None

    def kill(self):
        if self.proc and self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait(5)

    def connections(self) -> int:
        """Number of client connections the broker currently has (via $SYS)."""
        r = subprocess.run(["mosquitto_sub", "-h", "127.0.0.1", "-p", str(TCP), "-t", "$SYS/broker/clients/connected",
                            "-C", "1", "-W", "3"], capture_output=True, text=True)
        try:
            return int(r.stdout.strip()) - 1  # minus this mosquitto_sub
        except ValueError:
            return -1


# ---------------------------------------------------------------------------
# Process sampling


def _proc_cpu_ticks(pid) -> int | None:
    try:
        with open(f"/proc/{pid}/stat") as f:
            parts = f.read().rsplit(")", 1)[1].split()
        return int(parts[11]) + int(parts[12])
    except Exception:
        return None


def _status(pid) -> dict:
    out = {}
    try:
        with open(f"/proc/{pid}/status") as f:
            for line in f:
                k, _, v = line.partition(":")
                if k in ("VmRSS", "VmHWM", "Threads"):
                    out[k] = int(v.split()[0])
    except Exception:
        pass
    return out


def _threads(pid) -> dict[int, tuple[str, int]]:
    res = {}
    try:
        for tid in os.listdir(f"/proc/{pid}/task"):
            try:
                with open(f"/proc/{pid}/task/{tid}/stat") as f:
                    raw = f.read()
                comm = raw[raw.index("(") + 1:raw.rindex(")")]
                parts = raw.rsplit(")", 1)[1].split()
                res[int(tid)] = (comm, int(parts[11]) + int(parts[12]))
            except Exception:
                continue
    except Exception:
        pass
    return res


def broker_sockets(pid: int, port: int = TCP) -> int:
    """ESTABLISHED TCP connections from process `pid` to the broker port."""
    inodes = set()
    try:
        for fd in os.listdir(f"/proc/{pid}/fd"):
            try:
                link = os.readlink(f"/proc/{pid}/fd/{fd}")
            except OSError:
                continue
            if link.startswith("socket:["):
                inodes.add(link[8:-1])
    except Exception:
        return -1
    count = 0
    for table in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            lines = open(table).read().splitlines()[1:]
        except Exception:
            continue
        for line in lines:
            parts = line.split()
            if parts[3] == "01" and int(parts[2].split(":")[1], 16) == port and parts[9] in inodes:
                count += 1
    return count


def pids_of(pattern: str) -> list[int]:
    r = subprocess.run(["pgrep", "-f", pattern], capture_output=True, text=True)
    return [int(p) for p in r.stdout.split()]


def thread_group(comm: str, tid: int, pid: int) -> str:
    if tid == pid:
        return "main"
    if comm.startswith("mqtt-io"):
        return "mqtt_io"
    return re.sub(r"[\d\-_#]+$", "", comm) or "other"


class Sampler(threading.Thread):
    """Samples every `interval` seconds; rows are kept in memory and appended to `path`."""

    def __init__(self, pid: int, path: Path, interval: float = 1.0, broker: QuietBroker | None = None):
        super().__init__(daemon=True)
        self.pid, self.path, self.interval, self.broker = pid, path, interval, broker
        self.rows: list[dict] = []
        self.phase = "start"
        self._halt = threading.Event()
        self._others = {
            "dbus": pids_of("dbus-daemon"),
            "atspi": pids_of("at-spi"),
            "xvfb": pids_of(f"Xvfb {DISPLAY}"),
        }

    def stop(self):
        self._halt.set()
        self.join(5)

    def run(self):
        last_t = time.time()
        last_total = _proc_cpu_ticks(self.pid) or 0
        last_threads = _threads(self.pid)
        last_other: dict[str, int] = {}

        def other_ticks():
            d = {}
            bpid = self.broker.pid if self.broker else None
            d["broker"] = _proc_cpu_ticks(bpid) or 0 if bpid else 0
            d["loadgen"] = sum(_proc_cpu_ticks(p) or 0 for p in pids_of("mqtt-load"))
            for k, ps in self._others.items():
                d[k] = sum(_proc_cpu_ticks(p) or 0 for p in ps)
            return d

        last_other = other_ticks()
        with open(self.path, "a") as f:
            while not self._halt.wait(self.interval):
                now = time.time()
                dt = now - last_t
                total = _proc_cpu_ticks(self.pid)
                if total is None:
                    row = dict(t=round(now, 3), phase=self.phase, dead=True)
                    self.rows.append(row)
                    f.write(json.dumps(row) + "\n")
                    f.flush()
                    break
                st = _status(self.pid)
                threads = _threads(self.pid)
                groups: dict[str, float] = {}
                for tid, (comm, ticks) in threads.items():
                    prev = last_threads.get(tid, (comm, ticks))[1] if tid in last_threads else 0
                    g = thread_group(comm, tid, self.pid)
                    groups[g] = groups.get(g, 0.0) + (ticks - prev) / HZ / dt * 100
                oth = other_ticks()
                try:
                    fds = len(os.listdir(f"/proc/{self.pid}/fd"))
                except Exception:
                    fds = -1
                row = dict(
                    t=round(now, 3), phase=self.phase,
                    cpu=round((total - last_total) / HZ / dt * 100, 1),
                    rss_mb=round(st.get("VmRSS", 0) / 1024, 1), hwm_mb=round(st.get("VmHWM", 0) / 1024, 1),
                    threads=st.get("Threads", 0), fds=fds,
                    **{f"th_{k}": round(v, 1) for k, v in sorted(groups.items()) if v >= 0.5 or k in ("main", "mqtt_io")},
                    **{f"{k}_cpu": round((oth[k] - last_other.get(k, oth[k])) / HZ / dt * 100, 1) for k in oth},
                )
                self.rows.append(row)
                f.write(json.dumps(row) + "\n")
                f.flush()
                last_t, last_total, last_threads, last_other = now, total, threads, oth

    def window(self, phase: str) -> list[dict]:
        return [r for r in self.rows if r.get("phase") == phase and not r.get("dead")]

    def summary(self, phase: str) -> dict:
        rows = self.window(phase)
        if not rows:
            return {}
        def avg(k):
            v = [r.get(k, 0) for r in rows]
            return round(sum(v) / len(v), 1)
        def mx(k):
            return max(r.get(k, 0) for r in rows)
        keys = sorted({k for r in rows for k in r if k.startswith("th_") or k.endswith("_cpu")})
        return dict(samples=len(rows), cpu_avg=avg("cpu"), cpu_max=mx("cpu"),
                    rss_start=rows[0]["rss_mb"], rss_end=rows[-1]["rss_mb"], rss_max=mx("rss_mb"),
                    threads=rows[-1]["threads"], fds=rows[-1]["fds"],
                    **{k + "_avg": avg(k) for k in keys})


# ---------------------------------------------------------------------------
# Load generator


class LoadGen:
    def __init__(self, mode: str = "pub", log_name: str | None = None, **kw):
        args = [LOADGEN, mode, "--port", str(TCP)]
        for k, v in kw.items():
            args += [f"--{k.replace('_', '-')}", str(v)]
        self.args = args
        self.lines: list[dict] = []
        self.log_path = OUT / (log_name or f"loadgen-{mode}-{int(time.time() * 1000)}.jsonl")
        self.proc = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        self.started = time.time()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        with open(self.log_path, "a") as f:
            for line in self.proc.stdout:
                f.write(line)
                try:
                    self.lines.append(json.loads(line))
                except Exception:
                    pass

    def wait(self, timeout: float | None = None) -> dict:
        self.proc.wait(timeout)
        time.sleep(0.2)
        return self.final()

    def final(self) -> dict:
        for line in reversed(self.lines):
            if line.get("done"):
                return line
        return {}

    def stop(self):
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGINT)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()


# ---------------------------------------------------------------------------
# Data files


NOW = "2025-01-01T00:00:00Z"
SERVER = dict(username="", password="", client_id="", keep_alive=60, clean_session=True, use_tls=False,
              ca_cert="", client_cert="", client_key="", created_at=NOW, updated_at=NOW)
TAG_SCRIPT = ('async function process(payload, topic) {\n'
              '  try { const o = JSON.parse(payload); o.handled_by = "script"; return o; }\n'
              '  catch (e) { return payload; }\n}')


def server(i: int, name: str | None = None, version: str = "5.0") -> dict:
    return dict(SERVER, id=i, name=name or f"S{i:03d}", host="127.0.0.1", port=TCP, protocol_version=version)


def write_data(data_dir: Path, *, servers, subscriptions=(), templates=(), scripts=(), messages=()):
    data_dir.mkdir(parents=True, exist_ok=True)
    data = dict(servers=list(servers), subscriptions=list(subscriptions), messages=list(messages),
                templates=list(templates), scripts=list(scripts),
                next_server_id=max([s["id"] for s in servers] or [0]),
                next_subscription_id=max([s["id"] for s in subscriptions] or [0]),
                next_message_id=max([m["id"] for m in messages] or [0]),
                next_template_id=max([t["id"] for t in templates] or [0]),
                next_script_id=max([s["id"] for s in scripts] or [0]))
    path = data_dir / "data.yaml"
    path.write_text(yaml.safe_dump(data, allow_unicode=True, sort_keys=False), encoding="utf-8")
    return path


def sub(i, server_id, topic, qos=0, active=True):
    return dict(id=i, server_id=server_id, topic=topic, qos=qos, is_active=active, created_at=NOW)


def template(i, server_id, name, topic, payload, ptype="json", qos=0, category="perf"):
    return dict(id=i, server_id=server_id, name=name, topic=topic, payload=payload, payload_type=ptype, qos=qos,
                retain=False, description=None, category=category, use_count=0, last_used_at=None,
                created_at=NOW, updated_at=NOW)


def script(i, server_id, name, code, kind="after_receive", enabled=True):
    return dict(id=i, server_id=server_id, name=name, script_type=kind, enabled=enabled, description=None, code=code,
                created_at=NOW, updated_at=NOW)


# ---------------------------------------------------------------------------
# App + UI probes


ATSPI_LOCK = threading.RLock()  # pyatspi is used from the main thread and the LagWatcher thread


class PerfApp(App):
    def __init__(self, data_dir: Path, tag: str, extra_env: dict | None = None):
        shots = OUT / "shots"
        shots.mkdir(parents=True, exist_ok=True)
        super().__init__(BIN, data_dir, DISPLAY, shots, OUT / f"app-{tag}.log")
        self.env.update(extra_env or {})
        self._cache: dict[str, object] = {}
        self._list = None

    def _root(self):
        with ATSPI_LOCK:
            return super()._root()

    def nodes(self):
        with ATSPI_LOCK:
            return super().nodes()

    def timed_start(self) -> dict:
        """Start the app; returns seconds until the window is in the a11y tree and until a server row shows."""
        t0 = time.time()
        env = dict(self.env, MINI_MQTT_DATA_DIR=str(self.data_dir), RUST_LOG="warn,mini_mqtt_client=info")
        self.proc = subprocess.Popen([str(self.binary)], env=env, stdout=open(self.log_path, "ab"),
                                     stderr=subprocess.STDOUT)
        self._origin = None
        wait_until(lambda: self._root() is not None and self.find_all(role="frame"), 120, interval=0.05,
                   desc="app accessibility tree")
        t_frame = time.time() - t0
        wait_until(lambda: self.find_all(role="list item", pattern=r"^服务器 "), 120, interval=0.05,
                   desc="server rows")
        t_rows = time.time() - t0
        wait_until(self.window_origin, 10, desc="app window")
        time.sleep(0.5)
        return dict(frame_s=round(t_frame, 3), server_rows_s=round(t_rows, 3))

    # -- cheap node access -----------------------------------------------------

    def node(self, name: str, refresh=False):
        n = self._cache.get(name)
        if n is None or refresh:
            n = self.find(name=name, timeout=30)
            self._cache[name] = n
        return n

    def node_checked(self, name: str) -> bool:
        with ATSPI_LOCK:
            try:
                return self.node(name).checked
            except Exception:
                return self.node(name, refresh=True).checked

    def raw_click(self, node, dx=0.5, dy=0.5):
        with ATSPI_LOCK:
            x, y, w, h = node.extents()
        ox, oy = self.window_origin()
        self.xdo("mousemove", str(ox + x + int(w * dx)), str(oy + y + int(h * dy)), "click", "1")

    def filter_probe(self, timeout: float = 120.0) -> dict:
        """Click 消息过滤 发布 then 消息过滤 全部; seconds until each shows as pressed."""
        out = {}
        for name in ("消息过滤 发布", "消息过滤 全部"):
            node = self.node(name)
            t0 = time.time()
            self.raw_click(node)
            deadline = t0 + timeout
            ok = False
            while time.time() < deadline:
                try:
                    if self.node_checked(name):
                        ok = True
                        break
                except Exception:
                    pass
                time.sleep(0.01)
            out[name.split()[-1]] = round(time.time() - t0, 3) if ok else None
        return out

    # -- message list -----------------------------------------------------------

    def _message_nodes(self):
        return [n for n in self.nodes() if n.role == "list item" and n.name.startswith("消息 ")]

    def top_message(self) -> str | None:
        """Accessible name of the newest (top) message row."""
        with ATSPI_LOCK:
            return self._top_message()

    def _top_message(self) -> str | None:
        if self._list is not None:
            try:
                for i in range(min(self._list.childCount, 4)):
                    item = self._list.getChildAtIndex(i)
                    stack = [item]
                    while stack:
                        acc = stack.pop(0)
                        if acc is None:
                            continue
                        if acc.getRoleName() == "list item" and (acc.name or "").startswith("消息 "):
                            return acc.name
                        stack.extend(acc.getChildAtIndex(j) for j in range(acc.childCount))
            except Exception:
                self._list = None
        nodes = self._message_nodes()
        if not nodes:
            return None
        try:
            # cache the list container: the ancestor holding the row wrappers
            self._list = nodes[0].acc.parent.parent
        except Exception:
            self._list = None
        return nodes[0].name

    def message_count(self) -> str | None:
        with ATSPI_LOCK:
            return self._message_count()

    def _message_count(self) -> str | None:
        try:
            n = self._cache.get("count")
            name = n.acc.name if n is not None else None
            if not name or not name.startswith("消息数 "):
                n = self.find(role="status bar", pattern=r"^消息数 ", timeout=10)
                self._cache["count"] = n
                name = n.acc.name
            return name.removeprefix("消息数 ")
        except Exception:
            return None

    def connect_and_wait(self, timeout=20):
        self.click(Query(role="push button", name="连接"))
        self.find(role="status bar", pattern=r"^连接状态 已连接", timeout=timeout)
        time.sleep(0.5)


SEQ_RX = re.compile(r'seq["=:]+(\d+)\D+?ts["=:]+(\d+)')


def parse_seq(name: str | None):
    if not name:
        return None
    m = SEQ_RX.search(name)
    return (int(m.group(1)), int(m.group(2))) if m else None


class LagWatcher(threading.Thread):
    """Polls the top message row every `interval` s: newest displayed seq and display lag."""

    def __init__(self, app: PerfApp, interval: float = 0.5):
        super().__init__(daemon=True)
        self.app, self.interval = app, interval
        self.samples: list[dict] = []
        self._halt = threading.Event()
        self.phase = "start"

    def run(self):
        while not self._halt.wait(self.interval):
            t = time.time()
            try:
                name = self.app.top_message()
            except Exception:
                name = None
            dt_probe = time.time() - t
            parsed = parse_seq(name)
            if parsed:
                seq, ts = parsed
                self.samples.append(dict(t=round(t, 3), phase=self.phase, seq=seq, lag_s=round(t - ts / 1000, 3),
                                         probe_s=round(dt_probe, 3)))
            else:
                self.samples.append(dict(t=round(t, 3), phase=self.phase, seq=None, probe_s=round(dt_probe, 3)))

    def stop(self):
        self._halt.set()
        self.join(10)

    def last_seq(self):
        for s in reversed(self.samples):
            if s.get("seq") is not None:
                return s["seq"]
        return None


def wait_seq(app: PerfApp, seq: int, timeout: float) -> float | None:
    """Wait until the top row shows a seq >= `seq`; returns the time it happened or None."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        p = parse_seq(app.top_message())
        if p and p[0] >= seq:
            return time.time()
        time.sleep(0.1)
    return None


def save(name: str, obj):
    path = OUT / f"{name}.json"
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2))
    log("saved", path)
    return path


def app_log_issues(app: PerfApp) -> list[str]:
    issues = []
    for p in [app.log_path, *sorted((app.data_dir / "logs").glob("*.log"))]:
        if p.exists():
            for line in p.read_text(errors="replace").splitlines():
                if re.search(r"panic|thread '.*' panicked|ERROR|error|Error", line):
                    issues.append(f"{p.name}: {line[:300]}")
    return issues[:50]
