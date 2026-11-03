"""Windows backend of the end-to-end test driver (see driver.py).

The real application runs on the desktop, moved to the monitor chosen by
E2E_MONITOR (default: the largest one). Widgets are located through UI
Automation: AccessKit exports the same accessible names as on Linux, and the
ARIA roles are translated to the AT-SPI role names the scenarios use.

Input never touches the real mouse or the keyboard focus, so the desktop stays
usable while the tests run: clicks are mouse messages posted to the
application window, text arrives as WM_CHAR, keys as WM_KEYDOWN / WM_KEYUP,
and modifiers are applied through the window thread's keyboard state (the way
AutoHotkey's ControlSend does it). The clipboard is the system clipboard.
"""

from __future__ import annotations

import ctypes
import os
import subprocess
import time
from contextlib import contextmanager
from ctypes import wintypes as wt
from dataclasses import dataclass
from pathlib import Path

import comtypes
import comtypes.client
from PIL import ImageGrab

import driver
from driver import E2EError, check, wait_until

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

# Physical pixels everywhere: UI Automation rectangles, window positions, message coordinates.
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # PER_MONITOR_AWARE_V2

comtypes.client.GetModule("UIAutomationCore.dll")
from comtypes.gen import UIAutomationClient as uia  # noqa: E402

WNDENUMPROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
MONITORENUMPROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HMONITOR, wt.HDC, ctypes.POINTER(wt.RECT), wt.LPARAM)


class MONITORINFO(ctypes.Structure):
    _fields_ = [("cbSize", wt.DWORD), ("rcMonitor", wt.RECT), ("rcWork", wt.RECT), ("dwFlags", wt.DWORD)]


def _sig(fn, args, res=wt.BOOL):
    fn.argtypes, fn.restype = args, res


_sig(user32.EnumWindows, [WNDENUMPROC, wt.LPARAM])
_sig(user32.GetWindowThreadProcessId, [wt.HWND, ctypes.POINTER(wt.DWORD)], wt.DWORD)
_sig(user32.IsWindowVisible, [wt.HWND])
_sig(user32.IsIconic, [wt.HWND])
_sig(user32.GetWindowTextW, [wt.HWND, wt.LPWSTR, ctypes.c_int], ctypes.c_int)
_sig(user32.GetWindowRect, [wt.HWND, ctypes.POINTER(wt.RECT)])
_sig(user32.ClientToScreen, [wt.HWND, ctypes.POINTER(wt.POINT)])
_sig(user32.SetWindowPos, [wt.HWND, wt.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wt.UINT])
_sig(user32.PostMessageW, [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM])
_sig(user32.SendMessageTimeoutW,
     [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM, wt.UINT, wt.UINT, ctypes.POINTER(ctypes.c_size_t)], ctypes.c_ssize_t)
_sig(user32.GetDpiForWindow, [wt.HWND], wt.UINT)
_sig(user32.EnumDisplayMonitors, [wt.HDC, ctypes.POINTER(wt.RECT), MONITORENUMPROC, wt.LPARAM])
_sig(user32.GetMonitorInfoW, [wt.HMONITOR, ctypes.POINTER(MONITORINFO)])
_sig(user32.AttachThreadInput, [wt.DWORD, wt.DWORD, wt.BOOL])
_sig(user32.GetKeyboardState, [ctypes.POINTER(ctypes.c_ubyte)])
_sig(user32.SetKeyboardState, [ctypes.POINTER(ctypes.c_ubyte)])
_sig(user32.MapVirtualKeyW, [wt.UINT, wt.UINT], wt.UINT)
_sig(user32.OpenClipboard, [wt.HWND])
_sig(user32.CloseClipboard, [])
_sig(user32.EmptyClipboard, [])
_sig(user32.GetClipboardData, [wt.UINT], wt.HANDLE)
_sig(user32.SetClipboardData, [wt.UINT, wt.HANDLE], wt.HANDLE)
_sig(kernel32.GlobalAlloc, [wt.UINT, ctypes.c_size_t], wt.HGLOBAL)
_sig(kernel32.GlobalLock, [wt.HGLOBAL], ctypes.c_void_p)
_sig(kernel32.GlobalUnlock, [wt.HGLOBAL])
_sig(kernel32.GetCurrentThreadId, [], wt.DWORD)

WM_NULL, WM_CLOSE, WM_KEYDOWN, WM_KEYUP, WM_CHAR = 0x0000, 0x0010, 0x0100, 0x0101, 0x0102
WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP = 0x0200, 0x0201, 0x0202, 0x0204, 0x0205
MK_LBUTTON, MK_RBUTTON = 0x0001, 0x0002
SWP_NOSIZE, SWP_NOZORDER, SWP_NOACTIVATE = 0x0001, 0x0004, 0x0010
SMTO_NORMAL, CF_UNICODETEXT, GMEM_MOVEABLE = 0x0000, 13, 0x0002
CREATE_NO_WINDOW = 0x08000000

KEYS = {
    "escape": 0x1B, "return": 0x0D, "enter": 0x0D, "tab": 0x09, "backspace": 0x08, "delete": 0x2E,
    "end": 0x23, "home": 0x24, "left": 0x25, "up": 0x26, "right": 0x27, "down": 0x28, "space": 0x20,
    "prior": 0x21, "next": 0x22,
}
EXTENDED_KEYS = {0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2E}
# (generic, left-hand) virtual keys
MODIFIERS = {"ctrl": (0x11, 0xA2), "control": (0x11, 0xA2), "shift": (0x10, 0xA0)}

# ARIA role (as exported by AccessKit) -> the AT-SPI role name the scenarios use.
ROLES = {
    "alert": "notification", "alertdialog": "alert", "button": "push button", "checkbox": "check box",
    "switch": "toggle button", "combobox": "combo box", "dialog": "dialog", "heading": "heading",
    "img": "image", "link": "link", "list": "list", "listbox": "list box", "listitem": "list item",
    "option": "list item", "menu": "menu", "menubar": "menu bar", "menuitem": "menu item",
    "menuitemcheckbox": "check menu item", "menuitemradio": "radio menu item", "progressbar": "progress bar",
    "radio": "radio button", "radiogroup": "panel", "group": "panel", "scrollbar": "scroll bar",
    "slider": "slider", "status": "status bar", "tab": "page tab", "tablist": "page tab list",
    "textbox": "entry", "searchbox": "entry", "tooltip": "tool tip", "generic": "section",
    "region": "landmark", "separator": "separator", "toolbar": "tool bar", "tree": "tree",
    "treeitem": "tree item", "grid": "table", "table": "table", "row": "table row", "cell": "table cell",
}
# Fallback for nodes without an ARIA role, by UIA control type.
CONTROL_TYPES = {
    50000: "push button", 50004: "entry", 50020: "label", 50025: "section", 50026: "panel",
    50030: "document frame", 50032: "frame", 50033: "panel", 50037: "title bar",
}

P_RECT, P_CONTROL_TYPE, P_NAME, P_FOCUSED, P_FOCUSABLE, P_ENABLED = 30001, 30003, 30005, 30008, 30009, 30010
P_HAS_TOGGLE, P_SELECTED, P_TOGGLE, P_ARIA_ROLE = 30041, 30079, 30086, 30101


def _plain(value):
    """UIA returns a sentinel COM object for properties the element does not support."""
    return value if isinstance(value, (bool, int, float, str)) else None


def _lparam_xy(x: int, y: int) -> int:
    return ((y & 0xFFFF) << 16) | (x & 0xFFFF)


def _monitors() -> list[dict]:
    found = []

    def collect(hmon, _hdc, _rect, _data):
        info = MONITORINFO(cbSize=ctypes.sizeof(MONITORINFO))
        user32.GetMonitorInfoW(hmon, ctypes.byref(info))
        m, w = info.rcMonitor, info.rcWork
        found.append(dict(rect=(m.left, m.top, m.right, m.bottom), work=(w.left, w.top, w.right, w.bottom),
                          primary=bool(info.dwFlags & 1)))
        return True

    user32.EnumDisplayMonitors(None, None, MONITORENUMPROC(collect), 0)
    return found


def pick_monitor() -> dict:
    """E2E_MONITOR: `largest` (default), `primary`, or a 1-based index in enumeration order."""
    monitors = _monitors()
    choice = os.environ.get("E2E_MONITOR", "largest").strip().lower()
    if choice == "primary":
        return next(m for m in monitors if m["primary"])
    if choice.isdigit():
        return monitors[int(choice) - 1]

    def area(m):
        left, top, right, bottom = m["rect"]
        return (right - left) * (bottom - top)

    return max(monitors, key=area)


@dataclass(repr=False)
class Node(driver.Node):
    cached_states: set
    rect: tuple  # window (client) coordinates in logical pixels, like WINDOW_COORDS on X
    app: "App"

    @property
    def states(self) -> set[str]:
        return self.cached_states

    def extents(self):
        try:
            r = self.acc.CurrentBoundingRectangle
            return self.app._to_window(r.left, r.top, r.right, r.bottom)
        except Exception:
            return self.rect


class App(driver.App):
    # CF_UNICODETEXT ends at the first NUL. The desktop's clipboard is shared: a program reading it
    # (clipboard history, ...) can make a copy fail, which read() covers by pressing the keys again.
    READ_SENTINEL = "e2e-clipboard-sentinel"

    def __init__(self, binary: Path, data_dir: Path, display: str, shots_dir: Path, log_path: Path):
        self.binary, self.data_dir, self.display = binary, data_dir, display
        self.shots_dir, self.log_path = shots_dir, log_path
        self.proc: subprocess.Popen | None = None
        self.hwnd = None
        self._thread = 0
        self.env = dict(os.environ)
        self.monitor = pick_monitor()
        self._uia = comtypes.client.CreateObject(uia.CUIAutomation, interface=uia.IUIAutomation)
        req = self._uia.CreateCacheRequest()
        for prop in (P_RECT, P_CONTROL_TYPE, P_NAME, P_FOCUSED, P_FOCUSABLE, P_ENABLED, P_HAS_TOGGLE, P_SELECTED,
                     P_TOGGLE, P_ARIA_ROLE):
            req.AddProperty(prop)
        req.TreeScope = uia.TreeScope_Subtree
        req.TreeFilter = self._uia.RawViewCondition
        self._cache = req

    # -- lifecycle ----------------------------------------------------------

    def start(self):
        env = dict(self.env, MINI_MQTT_DATA_DIR=str(self.data_dir), RUST_LOG="warn,mini_mqtt_client=info")
        log = open(self.log_path, "ab")
        self.proc = subprocess.Popen([str(self.binary)], env=env, stdout=log, stderr=subprocess.STDOUT,
                                     creationflags=CREATE_NO_WINDOW)
        self.hwnd = None
        self.hwnd = wait_until(self._find_window, 40, desc="app window")
        self._thread = user32.GetWindowThreadProcessId(self.hwnd, None)
        self._place_window()
        wait_until(lambda: len(self.nodes()) > 5, 40, desc="app accessibility tree")
        time.sleep(0.8)

    def stop(self):
        if self.proc and self.proc.poll() is None:
            if self.hwnd:
                user32.PostMessageW(self.hwnd, WM_CLOSE, 0, 0)
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        self.proc = None
        self.hwnd = None

    def _find_window(self):
        found = []

        def collect(hwnd, _):
            pid = wt.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            if pid.value == self.proc.pid and user32.IsWindowVisible(hwnd) and not user32.IsIconic(hwnd):
                title = ctypes.create_unicode_buffer(256)
                user32.GetWindowTextW(hwnd, title, 256)
                if title.value == self.WINDOW_TITLE:
                    found.append(hwnd)
            return True

        user32.EnumWindows(WNDENUMPROC(collect), 0)
        return found[0] if found else None

    def _window_rect(self) -> tuple[int, int, int, int]:
        r = wt.RECT()
        user32.GetWindowRect(self.hwnd, ctypes.byref(r))
        return r.left, r.top, r.right, r.bottom

    def _place_window(self):
        """Move the window (which opens centred on the primary monitor) to the test monitor.

        The window re-applies its initial placement once it is activated, and it is resized for the
        target monitor's scale factor (WM_DPICHANGED), so repeat until it stays centred there."""
        wl, wt_, wr, wb = self.monitor["work"]
        for _ in range(8):
            left, top, right, bottom = self._window_rect()
            w, h = right - left, bottom - top
            x, y = wl + max(0, (wr - wl - w) // 2), wt_ + max(0, (wb - wt_ - h) // 2)
            if abs(left - x) <= 2 and abs(top - y) <= 2:
                return
            user32.SetWindowPos(self.hwnd, None, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
            time.sleep(0.5)
        left, top, right, bottom = self._window_rect()
        cx, cy = (left + right) // 2, (top + bottom) // 2
        check(wl <= cx <= wr and wt_ <= cy <= wb, f"window not on the test monitor: {self._window_rect()}")

    # -- tree ---------------------------------------------------------------

    def _scale(self) -> float:
        return (user32.GetDpiForWindow(self.hwnd) or 96) / 96

    def _client_origin(self) -> tuple[int, int]:
        p = wt.POINT(0, 0)
        user32.ClientToScreen(self.hwnd, ctypes.byref(p))
        return p.x, p.y

    def _to_window(self, left, top, right, bottom, origin=None, scale=None):
        ox, oy = origin or self._client_origin()
        s = scale or self._scale()
        return (int((left - ox) / s), int((top - oy) / s), int((right - left) / s), int((bottom - top) / s))

    def _root(self):
        return self.hwnd if self.alive() else None

    def nodes(self) -> list[Node]:
        if not self.hwnd or not self.alive():
            return []
        try:
            root = self._uia.ElementFromHandle(self.hwnd).BuildUpdatedCache(self._cache)
        except comtypes.COMError:
            return []
        origin, scale = self._client_origin(), self._scale()
        out: list[Node] = []

        def walk(el, depth):
            try:
                aria = _plain(el.GetCachedPropertyValue(P_ARIA_ROLE)) or ""
                control = _plain(el.GetCachedPropertyValue(P_CONTROL_TYPE)) or 0
                role = "frame" if depth == 0 else ROLES.get(aria) or CONTROL_TYPES.get(control) or aria or str(control)
                toggle = _plain(el.GetCachedPropertyValue(P_TOGGLE)) if _plain(el.GetCachedPropertyValue(P_HAS_TOGGLE)) else None
                if role == "push button" and toggle is not None:
                    role = "toggle button"
                states = set()
                if _plain(el.GetCachedPropertyValue(P_ENABLED)):
                    states |= {"enabled", "sensitive"}
                if _plain(el.GetCachedPropertyValue(P_FOCUSABLE)):
                    states.add("focusable")
                if _plain(el.GetCachedPropertyValue(P_FOCUSED)):
                    states.add("focused")
                if toggle == 1:
                    states.add("pressed" if role == "toggle button" else "checked")
                if _plain(el.GetCachedPropertyValue(P_SELECTED)):
                    states.add("selected")
                    if role == "radio button":
                        states.add("checked")
                r = el.CachedBoundingRectangle
                rect = self._to_window(r.left, r.top, r.right, r.bottom, origin, scale)
                out.append(Node(role, _plain(el.GetCachedPropertyValue(P_NAME)) or "", depth, el, states, rect, self))
                children = el.GetCachedChildren()
                for i in range(children.Length if children else 0):
                    walk(children.GetElement(i), depth + 1)
            except comtypes.COMError:
                pass

        walk(root, 0)
        return out

    # -- input --------------------------------------------------------------

    def _post(self, msg: int, wparam: int, lparam: int):
        check(user32.PostMessageW(self.hwnd, msg, wparam, lparam), f"PostMessage {msg:#x} failed")

    def _drain(self):
        """Wait until the window thread has handled the messages posted so far.

        A sent message is handled the next time the thread looks at its queue, before the posted
        ones; the second round trip therefore returns only after the first posted message ran."""
        result = ctypes.c_size_t()
        for _ in range(3):
            user32.SendMessageTimeoutW(self.hwnd, WM_NULL, 0, 0, SMTO_NORMAL, 10000, ctypes.byref(result))
            time.sleep(0.02)

    def xdo(self, *args: str):
        raise E2EError("xdotool is not available on Windows")

    def window_origin(self):
        return (0, 0)

    def click_window(self, x: int, y: int, button: int = 1):
        s = self._scale()
        lp = _lparam_xy(round(x * s), round(y * s))
        down, up, flag = (WM_RBUTTONDOWN, WM_RBUTTONUP, MK_RBUTTON) if button == 3 else (WM_LBUTTONDOWN, WM_LBUTTONUP, MK_LBUTTON)
        # Controls revealed on hover only get a hitbox in the frame drawn after the pointer
        # arrived: let one render before pressing, as the real pointer does on X.
        self._post(WM_MOUSEMOVE, 0, lp)
        self._drain()
        time.sleep(0.12)
        self._post(down, flag, lp)
        self._post(up, 0, lp)
        self._drain()

    def hover_away(self):
        s = self._scale()
        self._post(WM_MOUSEMOVE, 0, _lparam_xy(round(640 * s), round(5 * s)))
        self._drain()

    @contextmanager
    def _modifiers(self, names: list[str]):
        if not names:
            yield
            return
        me = kernel32.GetCurrentThreadId()
        check(user32.AttachThreadInput(me, self._thread, True), "AttachThreadInput failed")
        try:
            state = (ctypes.c_ubyte * 256)()
            user32.GetKeyboardState(state)
            saved = bytes(state)
            for name in names:
                for vk in MODIFIERS[name]:
                    state[vk] |= 0x80
            user32.SetKeyboardState(state)
            try:
                yield
            finally:
                user32.SetKeyboardState((ctypes.c_ubyte * 256).from_buffer_copy(saved))
        finally:
            user32.AttachThreadInput(me, self._thread, False)

    def key(self, *keys: str, settle: float = 0.2):
        for spec in keys:
            *mods, name = spec.split("+")
            vk = KEYS.get(name.lower())
            if vk is None:
                check(len(name) == 1 and name.isalnum(), f"unknown key {spec!r}")
                vk = ord(name.upper())
            lp = 1 | (user32.MapVirtualKeyW(vk, 0) << 16) | ((1 << 24) if vk in EXTENDED_KEYS else 0)
            with self._modifiers([m.lower() for m in mods]):
                self._post(WM_KEYDOWN, vk, lp)
                self._post(WM_KEYUP, vk, lp | (1 << 30) | (1 << 31))
                self._drain()
        time.sleep(settle)

    def type(self, text: str):
        if "\n" in text:
            self.paste(text)
            return
        units = text.encode("utf-16-le")
        for i in range(0, len(units), 2):
            self._post(WM_CHAR, int.from_bytes(units[i:i + 2], "little"), 1)
        self._drain()
        time.sleep(0.15)

    @contextmanager
    def _clipboard_open(self):
        deadline = time.time() + 3
        while not user32.OpenClipboard(None):
            check(time.time() < deadline, "clipboard is held by another program")
            time.sleep(0.02)
        try:
            yield
        finally:
            user32.CloseClipboard()

    def set_clipboard(self, text: str):
        data = text.encode("utf-16-le") + b"\0\0"
        with self._clipboard_open():
            user32.EmptyClipboard()
            handle = kernel32.GlobalAlloc(GMEM_MOVEABLE, len(data))
            ctypes.memmove(kernel32.GlobalLock(handle), data, len(data))
            kernel32.GlobalUnlock(handle)
            user32.SetClipboardData(CF_UNICODETEXT, handle)
        time.sleep(0.15)

    def clipboard(self) -> str:
        with self._clipboard_open():
            handle = user32.GetClipboardData(CF_UNICODETEXT)
            if not handle:
                return ""
            try:
                return ctypes.wstring_at(kernel32.GlobalLock(handle))
            finally:
                kernel32.GlobalUnlock(handle)

    # -- evidence -----------------------------------------------------------

    def screenshot(self, name: str) -> Path:
        path = self.shots_dir / f"{name}.png"
        try:
            ImageGrab.grab(bbox=self._window_rect(), all_screens=True).save(path)
        except Exception:
            pass
        return path
