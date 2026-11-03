"""End-to-end test driver for Mini MQTT Client.

The real application binary runs on an X server. Widgets are located through
the accessibility tree (AT-SPI, exported by AccessKit), and driven with real
X input events (xdotool): mouse clicks, key presses, clipboard paste. Results
are asserted on three independent surfaces:

* the UI itself (accessible names / states of widgets),
* the MQTT broker (a mosquitto_sub spy sees exactly what the app published),
* the files the app persists (data.yaml, config.yaml, logs).

On Windows, driver_win.py replaces the X / AT-SPI parts of `App` with UI
Automation and window messages; everything else here is shared.
"""

from __future__ import annotations

import os
import re
import shutil
import signal
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable

import yaml

try:
    import pyatspi
except ImportError:  # Windows: driver_win.py provides the accessibility backend
    pyatspi = None


class E2EError(AssertionError):
    """A failed expectation."""


def wait_until(fn: Callable, timeout: float = 8.0, interval: float = 0.15, desc: str = ""):
    """Poll `fn` until it returns a truthy value; raise E2EError on timeout."""
    deadline = time.time() + timeout
    last_error = None
    while time.time() < deadline:
        try:
            result = fn()
            if result:
                return result
        except E2EError:
            raise
        except Exception as err:  # transient AT-SPI errors while the tree changes
            last_error = err
        time.sleep(interval)
    extra = f" (last error: {last_error!r})" if last_error else ""
    raise E2EError(f"timed out after {timeout:.1f}s waiting for {desc}{extra}")


def check(condition, message: str):
    if not condition:
        raise E2EError(message)


def run(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, check=True, capture_output=True, **kw)


# ---------------------------------------------------------------------------
# Accessibility tree


@dataclass
class Node:
    role: str
    name: str
    depth: int
    acc: object

    @property
    def states(self) -> set[str]:
        try:
            return {pyatspi.stateToString(s) for s in self.acc.getState().getStates()}
        except Exception:
            return set()

    @property
    def checked(self) -> bool:
        s = self.states
        return "checked" in s or "pressed" in s

    @property
    def enabled(self) -> bool:
        return "enabled" in self.states and "sensitive" in self.states

    def extents(self):
        e = self.acc.queryComponent().getExtents(pyatspi.WINDOW_COORDS)
        return e.x, e.y, e.width, e.height

    def __repr__(self) -> str:
        return f"<{self.role} {self.name!r}>"


class Query:
    """Match accessible nodes by role and name (exact, substring or regex)."""

    def __init__(self, role=None, name=None, contains=None, pattern=None, index=0):
        self.role, self.name, self.contains, self.index = role, name, contains, index
        self.pattern = re.compile(pattern) if isinstance(pattern, str) else pattern

    def matches(self, node: Node) -> bool:
        if self.role and node.role != self.role:
            return False
        if self.name is not None and node.name != self.name:
            return False
        if self.contains is not None and self.contains not in node.name:
            return False
        if self.pattern is not None and not self.pattern.search(node.name):
            return False
        return True

    def __repr__(self) -> str:
        parts = [f"{k}={v!r}" for k, v in vars(self).items() if v not in (None, 0)]
        return f"Query({', '.join(parts)})"


# ---------------------------------------------------------------------------
# The application under test


class App:
    WINDOW_TITLE = "Mini MQTT Client"
    APP_NAME = "mini-mqtt-client"

    def __init__(self, binary: Path, data_dir: Path, display: str, shots_dir: Path, log_path: Path):
        self.binary, self.data_dir, self.display = binary, data_dir, display
        self.shots_dir, self.log_path = shots_dir, log_path
        self.proc: subprocess.Popen | None = None
        self._origin = None
        self.env = dict(os.environ, DISPLAY=display)

    # -- lifecycle ----------------------------------------------------------

    def start(self):
        env = dict(self.env, MINI_MQTT_DATA_DIR=str(self.data_dir), RUST_LOG="warn,mini_mqtt_client=info")
        log = open(self.log_path, "ab")
        self.proc = subprocess.Popen([str(self.binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
        self._origin = None
        wait_until(lambda: self._root() is not None and self.find_all(role="frame"), 40, desc="app accessibility tree")
        wait_until(self.window_origin, 10, desc="app window")
        time.sleep(0.8)

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        self.proc = None
        # Let the accessibility registry drop the old application.
        wait_until(lambda: self._root() is None, 10, desc="app to leave the a11y registry")

    def alive(self) -> bool:
        return self.proc is not None and self.proc.poll() is None

    # -- tree ---------------------------------------------------------------

    def _root(self):
        desktop = pyatspi.Registry.getDesktop(0)
        for i in range(desktop.childCount):
            child = desktop.getChildAtIndex(i)
            if child is not None and child.name == self.APP_NAME:
                return child
        return None

    def nodes(self) -> list[Node]:
        root = self._root()
        out: list[Node] = []
        if root is None:
            return out

        def walk(acc, depth):
            try:
                out.append(Node(acc.getRoleName(), acc.name or "", depth, acc))
                for i in range(acc.childCount):
                    child = acc.getChildAtIndex(i)
                    if child is not None:
                        walk(child, depth + 1)
            except Exception:
                pass

        walk(root, 0)
        return out

    def find_all(self, *args, **kw) -> list[Node]:
        q = args[0] if args and isinstance(args[0], Query) else Query(*args, **kw)
        return [n for n in self.nodes() if q.matches(n)]

    def find(self, *args, timeout: float = 8.0, **kw) -> Node:
        q = args[0] if args and isinstance(args[0], Query) else Query(*args, **kw)

        def lookup():
            found = [n for n in self.nodes() if q.matches(n)]
            ok = len(found) > q.index if q.index >= 0 else len(found) >= -q.index
            return found[q.index] if ok else None

        try:
            return wait_until(lookup, timeout, desc=repr(q))
        except E2EError as err:
            raise E2EError(f"{err}\n--- visible nodes ---\n{self.dump()}") from None

    def find_in(self, scope: Query, target: Query, timeout: float = 8.0) -> Node:
        """The first node matching `target` inside the subtree of the last node matching `scope`."""

        def lookup():
            nodes = self.nodes()
            starts = [i for i, n in enumerate(nodes) if scope.matches(n)]
            if not starts:
                return None
            depth = nodes[starts[-1]].depth
            for n in nodes[starts[-1] + 1:]:
                if n.depth <= depth:
                    return None
                if target.matches(n):
                    return n
            return None

        try:
            return wait_until(lookup, timeout, desc=f"{target!r} in {scope!r}")
        except E2EError as err:
            raise E2EError(f"{err}\n--- visible nodes ---\n{self.dump()}") from None

    def exists(self, *args, **kw) -> bool:
        return bool(self.find_all(*args, **kw))

    def wait_gone(self, *args, timeout: float = 8.0, **kw):
        q = Query(*args, **kw)
        wait_until(lambda: not self.find_all(q), timeout, desc=f"{q} to disappear")

    def names(self, role: str) -> list[str]:
        return [n.name for n in self.find_all(role=role)]

    def dump(self) -> str:
        return "\n".join(f"{'  ' * n.depth}{n.role}: {n.name!r}" for n in self.nodes())

    # -- input --------------------------------------------------------------

    def xdo(self, *args: str):
        run(["xdotool", *args], env=self.env)

    def window_origin(self):
        if self._origin is None:
            out = subprocess.run(
                ["xdotool", "search", "--name", self.WINDOW_TITLE, "getwindowgeometry"],
                capture_output=True, text=True, env=self.env,
            ).stdout
            m = re.search(r"Position: (\d+),(\d+)", out)
            if not m:
                return None
            self._origin = (int(m.group(1)), int(m.group(2)))
        return self._origin

    def _covered(self, x: int, y: int) -> bool:
        """Is the window point under a toast notification?"""
        for n in self.find_all(role="notification"):
            try:
                nx, ny, nw, nh = n.extents()
            except Exception:
                continue
            if nx <= x <= nx + nw and ny <= y <= ny + nh:
                return True
        return False

    def click(self, target, *, dx: float = 0.5, dy: float = 0.5, button: int = 1, settle: float = 0.35, **kw):
        node = target if isinstance(target, Node) else self.find(target, **kw) if isinstance(target, Query) else self.find(**kw)
        x, y, w, h = node.extents()
        check(w > 0 and h > 0, f"{node} has no on-screen area")
        px_, py_ = x + int(w * dx), y + int(h * dy)
        if self._covered(px_, py_):
            # A user waits for the toast to go away before clicking under it.
            wait_until(lambda: not self._covered(px_, py_), 12, desc=f"toast covering {node} to disappear")
        # Dialogs and drawers slide in: click where the control ends up, not where it was a frame ago.
        x, y, w, h = self._settled(node)
        self.click_window(x + int(w * dx), y + int(h * dy), button)
        time.sleep(settle)
        return node

    def _settled(self, node: Node, timeout: float = 4.0):
        """The node's extents once two readings in a row agree."""
        last = node.extents()
        deadline = time.time() + timeout
        while time.time() < deadline:
            time.sleep(0.08)
            current = node.extents()
            if current == last:
                return current
            last = current
        raise E2EError(f"{node} is still moving: {last}")

    def click_window(self, x: int, y: int, button: int = 1):
        """Click at a point given in window coordinates."""
        ox, oy = self.window_origin()
        self.xdo("mousemove", str(ox + x), str(oy + y), "click", str(button))

    def hover_away(self):
        ox, oy = self.window_origin()
        self.xdo("mousemove", str(ox + 640), str(oy + 5))

    def key(self, *keys: str, settle: float = 0.2):
        for k in keys:
            self.xdo("key", "--clearmodifiers", k)
        time.sleep(settle)

    def type(self, text: str):
        if text.isascii() and "\n" not in text:
            self.xdo("type", "--delay", "12", text)
            time.sleep(0.15)
        else:
            self.paste(text)

    def set_clipboard(self, text: str):
        subprocess.run(["xclip", "-selection", "clipboard", "-i"], input=text.encode(), env=self.env, check=True)
        time.sleep(0.15)

    def clipboard(self) -> str:
        r = subprocess.run(["xclip", "-selection", "clipboard", "-o"], capture_output=True, env=self.env)
        return r.stdout.decode("utf-8", "replace")

    def paste(self, text: str):
        self.set_clipboard(text)
        self.key("ctrl+v", settle=0.3)

    def fill(self, target, text: str, **kw):
        """Replace the content of a text field."""
        node = self.click(target, **kw)
        self.key("ctrl+a")
        if text:
            self.type(text)
        else:
            self.key("BackSpace")
        return node

    def read(self, target, **kw) -> str:
        """Read a text field like a user would: select all and copy."""
        self.click(target, **kw)
        self.set_clipboard("\x00e2e-sentinel")
        self.key("ctrl+a", "ctrl+c", settle=0.3)
        value = self.clipboard()
        self.key("End")
        return "" if value == "\x00e2e-sentinel" else value

    def escape_all(self, times: int = 3):
        for _ in range(times):
            self.key("Escape", settle=0.25)
        time.sleep(0.3)

    # -- evidence -----------------------------------------------------------

    def screenshot(self, name: str) -> Path:
        path = self.shots_dir / f"{name}.png"
        ox, oy = self.window_origin() or (0, 0)
        xwd = subprocess.run(["xwd", "-root", "-silent"], capture_output=True, env=self.env).stdout
        subprocess.run(
            ["convert", "xwd:-", "-crop", f"1280x800+{ox}+{oy}", "+repage", f"png:{path}"],
            input=xwd, env=self.env, check=False,
        )
        return path


# ---------------------------------------------------------------------------
# Broker side


class Broker:
    def __init__(self, host: str, port: int):
        self.host, self.port = host, port

    def pub(self, topic: str, payload: bytes | str, qos: int = 0, retain: bool = False):
        data = payload.encode() if isinstance(payload, str) else payload
        cmd = ["mosquitto_pub", "-h", self.host, "-p", str(self.port), "-t", topic, "-q", str(qos), "-s"]
        if retain:
            cmd.append("-r")
        run(cmd, input=data)

    def clear_retained(self, topic: str):
        run(["mosquitto_pub", "-h", self.host, "-p", str(self.port), "-t", topic, "-r", "-n"])

    def fetch_retained(self, topic: str, timeout: int = 3):
        """Subscribe fresh and return (qos, retain, payload) of the retained message."""
        r = subprocess.run(
            ["mosquitto_sub", "-h", self.host, "-p", str(self.port), "-t", topic, "-q", "2",
             "-C", "1", "-W", str(timeout), "-F", "%q|%r|%x"],
            capture_output=True, text=True,
        )
        line = r.stdout.strip()
        if not line:
            return None
        qos, retain, hexpayload = line.split("|", 2)
        return int(qos), retain == "1", bytes.fromhex(hexpayload)


@dataclass
class Seen:
    topic: str
    qos: int
    retain: bool
    payload: bytes
    at: float


class Spy:
    """A mosquitto_sub on '#' recording everything published to the broker."""

    def __init__(self, broker: Broker):
        self.broker = broker
        self.messages: list[Seen] = []
        self._lock = threading.Lock()
        self.proc = subprocess.Popen(
            ["mosquitto_sub", "-h", broker.host, "-p", str(broker.port), "-t", "#", "-q", "2",
             "-F", "%t|%q|%r|%x", "-i", f"e2e-spy-{os.getpid()}"],
            stdout=subprocess.PIPE, text=True, bufsize=1,
        )
        threading.Thread(target=self._reader, daemon=True).start()
        time.sleep(0.5)

    def _reader(self):
        for line in self.proc.stdout:
            line = line.rstrip("\n")
            if line.count("|") < 3:
                continue
            topic, qos, retain, hexpayload = line.rsplit("|", 3)
            with self._lock:
                self.messages.append(Seen(topic, int(qos), retain == "1", bytes.fromhex(hexpayload), time.time()))

    def mark(self) -> int:
        with self._lock:
            return len(self.messages)

    def since(self, mark: int, topic: str | None = None) -> list[Seen]:
        with self._lock:
            msgs = self.messages[mark:]
        return [m for m in msgs if topic is None or m.topic == topic]

    def wait(self, mark: int, topic: str, count: int = 1, timeout: float = 6.0) -> list[Seen]:
        return wait_until(
            lambda: (lambda m: m if len(m) >= count else None)(self.since(mark, topic)),
            timeout, desc=f"{count} message(s) on {topic!r} at the broker",
        )

    def assert_silent(self, mark: int, topic: str, wait: float = 1.2):
        time.sleep(wait)
        msgs = self.since(mark, topic)
        check(not msgs, f"expected nothing on {topic!r}, broker saw {[m.payload for m in msgs]}")

    def stop(self):
        self.proc.terminate()


# ---------------------------------------------------------------------------
# Persisted files


def load_yaml(path: Path) -> dict:
    if not path.exists():
        return {}
    return yaml.safe_load(path.read_text(encoding="utf-8")) or {}


def which_all(*tools: str) -> list[str]:
    return [t for t in tools if shutil.which(t) is None]


def find_one(items: Iterable[dict], **match) -> dict | None:
    for item in items or []:
        if all(item.get(k) == v for k, v in match.items()):
            return item
    return None
