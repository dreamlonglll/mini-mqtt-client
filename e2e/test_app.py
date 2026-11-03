#!/usr/bin/env python3
"""End-to-end scenarios for Mini MQTT Client. Run through e2e/run.sh."""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
import time
import traceback
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from driver import App, Broker, E2EError, Query, Spy, check, find_one, load_yaml, wait_until  # noqa: E402

import yaml  # noqa: E402

WORK = Path(os.environ["E2E_WORK"])
DATA = WORK / "data"
SHOTS = WORK / "shots"
TCP = int(os.environ["E2E_TCP_PORT"])
WS = int(os.environ["E2E_WS_PORT"])
TLS = int(os.environ["E2E_TLS_PORT"])
CA_PEM = Path(os.environ["E2E_CA"]).read_text()
ONLY = {t.strip() for t in os.environ.get("E2E_ONLY", "").split(",") if t.strip()}

broker = Broker("127.0.0.1", TCP)
app = App(Path(os.environ["E2E_BIN"]), DATA, os.environ["E2E_DISPLAY"], SHOTS, WORK / "app.log")
spy: Spy | None = None

TESTS: list[tuple[str, str, callable]] = []


def scenario(test_id: str, title: str):
    def register(fn):
        TESTS.append((test_id, title, fn))
        return fn

    return register


# ---------------------------------------------------------------------------
# Seed data in the format written by the previous (Tauri) version


def seed():
    now = "2025-01-01T00:00:00Z"
    server = dict(username="", password="", client_id="", keep_alive=60, clean_session=True, use_tls=False,
                  ca_cert="", client_cert="", client_key="", client_key_password=None, created_at=now, updated_at=now)
    data = {
        "servers": [
            dict(server, id=1, name="本地 TCP", host="127.0.0.1", port=TCP, protocol_version="5.0"),
            dict(server, id=2, name="不可达", host="127.0.0.1", port=1, protocol_version="3.1.1"),
        ],
        "subscriptions": [
            dict(id=1, server_id=1, topic="e2e/#", qos=0, is_active=True, color=None, created_at=now),
            dict(id=2, server_id=1, topic="sensor/+/temp", qos=1, is_active=False, color="#F56C6C", created_at=now),
        ],
        "messages": [],
        "templates": [
            dict(id=1, server_id=1, name="开灯", topic="e2e/light", payload='{"action":"on","brightness":80}',
                 payload_type="json", qos=1, retain=False, description="打开客厅灯", category="照明",
                 use_count=3, last_used_at=None, created_at=now, updated_at=now),
            dict(id=2, server_id=1, name="心跳", topic="e2e/heartbeat", payload="01 02 03 FF",
                 payload_type="hex", qos=0, retain=False, description=None, category="系统",
                 use_count=1, last_used_at=None, created_at=now, updated_at=now),
        ],
        "scripts": [
            dict(id=1, server_id=1, name="接收标记", script_type="after_receive", enabled=True, description=None,
                 code='async function process(payload, topic) {\n'
                      '  try { const o = JSON.parse(payload); o.handled_by = "script"; return o; }\n'
                      '  catch (e) { return payload; }\n}',
                 created_at=now, updated_at=now),
        ],
        "env_variables": [
            dict(id=1, server_id=1, name="DEVICE_ID", value="dev-e2e", description="设备编号",
                 created_at=now, updated_at=now),
        ],
        "next_server_id": 2,
        "next_subscription_id": 2,
        "next_message_id": 0,
        "next_template_id": 2,
        "next_script_id": 1,
        "next_env_variable_id": 1,
    }
    (DATA / "data.yaml").write_text(yaml.safe_dump(data, allow_unicode=True, sort_keys=False), encoding="utf-8")


# ---------------------------------------------------------------------------
# Helpers expressed in the app's own vocabulary


def data() -> dict:
    return load_yaml(DATA / "data.yaml")


def config() -> dict:
    return load_yaml(DATA / "config.yaml")


def toast(contains: str, timeout: float = 8.0):
    """A notification message (exposed as a status node inside the alert)."""
    return app.find(role="status bar", contains=contains, timeout=timeout)


def status(text: str, timeout: float = 15.0):
    return app.find(role="status bar", pattern=rf"^连接状态 {re.escape(text)}", timeout=timeout)


def server_q(name: str) -> Query:
    return Query(role="list item", pattern=rf"^服务器 {re.escape(name)} ")


def select_server(name: str):
    app.click(server_q(name), dx=0.3)
    app.find(role="list item", pattern=rf"^服务器 {re.escape(name)} .* 当前$")


def messages() -> list[str]:
    return [n.name for n in app.find_all(role="list item") if n.name.startswith("消息 ")]


def wait_message(pattern: str, timeout: float = 8.0) -> str:
    rx = re.compile(pattern)
    return wait_until(lambda: next((m for m in messages() if rx.search(m)), None), timeout,
                      desc=f"message matching {pattern!r}; have {messages()[:6]}")


def assert_no_message(pattern: str, wait: float = 1.5):
    time.sleep(wait)
    rx = re.compile(pattern)
    hits = [m for m in messages() if rx.search(m)]
    check(not hits, f"unexpected message(s): {hits}")


def message_count() -> str:
    return app.find(role="status bar", pattern=r"^消息数 ").name.removeprefix("消息数 ")


def click(name: str, role: str | None = None, index: int = 0, **kw):
    return app.click(Query(role=role, name=name, index=index), **kw)


def pressed(name: str) -> bool:
    return app.find(name=name).checked


def ensure_checked(name: str, value: bool):
    node = app.find(name=name)
    if node.checked != value:
        app.click(node)
        wait_until(lambda: app.find(name=name).checked == value, 4, desc=f"{name} checked={value}")


def confirm(button: str):
    """Press the OK button of the confirmation dialog (the last one rendered)."""
    app.click(Query(role="push button", name=button, index=-1))


def close_dialogs():
    """Close dialogs and the template / environment drawers (sheets without a dialog role).

    Escape only reaches a dialog or drawer that holds focus, which is often not the case after
    a scenario failed half-way; from the second attempt on, click a close control instead."""
    for attempt in range(6):
        dialogs = app.find_all(role="dialog")
        drawer = app.find_all(name="搜索模板") or app.find_all(name="搜索环境变量")
        if not dialogs and not drawer:
            break
        closers = app.find_all(role="push button", name="Close") if dialogs and attempt >= 1 else []
        if closers:
            # The title-bar close button of the topmost dialog (overlays come last in the tree).
            app.click(closers[-1])
        elif drawer and not dialogs and attempt >= 1:
            # Click the drawer's overlay.
            ox, oy = app.window_origin()
            app.xdo("mousemove", str(ox + 300), str(oy + 450), "click", "1")
            time.sleep(0.5)
        else:
            app.key("Escape", settle=0.4)
    app.hover_away()


def publish(topic: str, payload: str, fmt: str = "JSON", qos: int = 0, retain: bool = False, expect_ok: bool = True):
    click(f"发布格式 {fmt}")
    app.fill(Query(name="发布 Topic"), topic)
    app.fill(Query(name="发布内容"), payload)
    click(f"发布 QoS QoS {qos}")
    ensure_checked("Retain", retain)
    click("发布", role="push button")
    if expect_ok:
        toast("消息已发布")


def open_scripts():
    click("管理脚本")
    app.find(name="脚本名称")


def new_script(name: str, code: str, kind: str = "发送前处理"):
    click("新增", role="push button")
    app.fill(Query(name="脚本名称"), name)
    click(kind, role="radio button")
    app.fill(Query(name="脚本代码"), code)
    click("保存", role="push button")


# ---------------------------------------------------------------------------
# Scenarios


@scenario("T01", "启动并加载旧版（Tauri）数据")
def t01():
    app.find(server_q("本地 TCP"))
    app.find(role="list item", name=f"服务器 本地 TCP mqtt://127.0.0.1:{TCP} 未连接 当前")
    app.find(role="list item", name="服务器 不可达 mqtt://127.0.0.1:1 未连接")
    app.find(role="list item", name="订阅 e2e/# Q0 已启用")
    app.find(role="list item", name="订阅 sensor/+/temp Q1 已停用")
    check(not app.find(name="启用订阅 sensor/+/temp").checked, "inactive subscription switch should be off")
    check(app.find(name="启用订阅 e2e/#").checked, "active subscription switch should be on")
    app.find(role="push button", name="开灯")
    app.find(role="push button", name="心跳")
    status("未连接")
    # gpui-component exposes a disabled button as "not focusable".
    check("focusable" not in app.find(role="push button", name="发布").states, "publish disabled while disconnected")
    check(pressed("发布格式 JSON") and pressed("发布 QoS QoS 0"), "default publish format / QoS")


@scenario("T02", "连接、自动订阅与接收后脚本")
def t02():
    click("连接")
    status("已连接")
    toast("已连接到 本地 TCP")
    app.find(role="push button", name="断开")
    check("focusable" in app.find(role="push button", name="发布").states, "publish enabled once connected")
    time.sleep(0.5)
    broker.pub("e2e/json", '{"temp":21.5}')
    wait_message(r'^消息 RCV e2e/json JSON Q0 \{"temp":21.5,"handled_by":"script"\}$')
    broker.pub("sensor/1/temp", "19")  # subscription is disabled
    assert_no_message(r"sensor/1/temp")


@scenario("T03", "接收：文本 / 二进制 / Retain / QoS")
def t03():
    broker.pub("e2e/text", "你好，MQTT 世界")
    wait_message(r"^消息 RCV e2e/text TEXT Q0 你好，MQTT 世界$")
    broker.pub("e2e/bin", bytes([0x00, 0x01, 0x02, 0xFF, 0xFE, 0x10]))
    # Bytes survive the enabled after-receive script untouched.
    wait_message(r"^消息 RCV e2e/bin BIN Q0 00 01 02 FF FE 10$")
    broker.pub("e2e/retained", "keep-me", retain=True)
    wait_message(r"^消息 RCV e2e/retained TEXT Q0 keep-me$")
    click("断开")
    status("未连接")
    click("连接")
    status("已连接")
    wait_message(r"^消息 RCV e2e/retained TEXT Q0 R keep-me$")
    broker.clear_retained("e2e/retained")


@scenario("T04", "订阅管理：启停、新增、校验、去重、删除")
def t04():
    click("启用订阅 sensor/+/temp")
    app.find(role="list item", name="订阅 sensor/+/temp Q1 已启用")
    check(find_one(data()["subscriptions"], topic="sensor/+/temp")["is_active"] is True, "is_active persisted")
    time.sleep(0.4)
    broker.pub("sensor/kitchen/temp", "22.5", qos=1)
    wait_message(r"^消息 RCV sensor/kitchen/temp TEXT Q1 22.5$")

    click("启用订阅 e2e/#")
    app.find(role="list item", name="订阅 e2e/# Q0 已停用")
    time.sleep(0.4)
    broker.pub("e2e/while-off", "x")
    assert_no_message(r"e2e/while-off")
    click("启用订阅 e2e/#")
    app.find(role="list item", name="订阅 e2e/# Q0 已启用")
    time.sleep(0.4)
    broker.pub("e2e/back-on", "y")
    wait_message(r"^消息 RCV e2e/back-on TEXT Q0 y$")

    click("新增订阅")
    app.fill(Query(name="订阅 Topic"), "home/+/light")
    click("订阅 QoS QoS 1")
    click("订阅", role="push button")
    toast("订阅已添加")
    app.find(role="list item", name="订阅 home/+/light Q1 已启用")
    sub = find_one(data()["subscriptions"], topic="home/+/light")
    check(sub and sub["qos"] == 1 and sub["server_id"] == 1, f"persisted subscription: {sub}")
    time.sleep(0.4)
    broker.pub("home/hall/light", "on", qos=1)
    wait_message(r"^消息 RCV home/hall/light TEXT Q1 on$")

    click("新增订阅")
    app.fill(Query(name="订阅 Topic"), "a/#/b")
    click("订阅", role="push button")
    toast("# 通配符只能在主题末尾")
    check(app.find_all(role="dialog"), "dialog must stay open on invalid topic")
    app.fill(Query(name="订阅 Topic"), "home/+/light")
    click("订阅", role="push button")
    toast("已存在订阅")
    close_dialogs()

    row = app.find(role="list item", name="订阅 home/+/light Q1 已启用")
    app.click(row, dx=0.5)  # hover reveals the delete button
    click("删除订阅 home/+/light")
    app.wait_gone(role="list item", name="订阅 home/+/light Q1 已启用")
    check(find_one(data()["subscriptions"], topic="home/+/light") is None, "subscription removed from data.yaml")


@scenario("T05", "发布：JSON/TEXT/HEX、QoS、Retain、格式校验")
def t05():
    mark = spy.mark()
    publish("e2e/pub/json", '{"a":1}', "JSON", qos=1, retain=True)
    seen = spy.wait(mark, "e2e/pub/json")[0]
    check(seen.payload == b'{"a":1}' and seen.qos == 1, f"broker got {seen}")
    retained = broker.fetch_retained("e2e/pub/json")
    check(retained and retained[1] and retained[2] == b'{"a":1}', f"retained on broker: {retained}")
    wait_message(r'^消息 PUB e2e/pub/json JSON Q1 R \{"a":1\}$')
    wait_message(r'^消息 RCV e2e/pub/json JSON Q\d \{"a":1,"handled_by":"script"\}$')
    broker.clear_retained("e2e/pub/json")

    mark = spy.mark()
    publish("e2e/pub/bad", "{bad", "JSON", expect_ok=False)
    toast("JSON 格式不正确")
    publish("e2e/+/x", "1", "TEXT", expect_ok=False)
    toast("发布主题不能包含通配符")
    publish("e2e/pub/hex", "zz", "HEX", expect_ok=False)
    toast("HEX 格式不正确")
    spy.assert_silent(mark, "e2e/pub/bad", wait=0.3)
    spy.assert_silent(mark, "e2e/pub/hex", wait=0.3)

    publish("e2e/pub/text", "中文 payload ✓", "TEXT", qos=2)
    seen = spy.wait(mark, "e2e/pub/text")[0]
    check(seen.payload == "中文 payload ✓".encode() and seen.qos == 2, f"broker got {seen}")

    publish("e2e/pub/hex", "48 65 6C 6C 6F", "HEX")
    seen = spy.wait(mark, "e2e/pub/hex")[0]
    check(seen.payload == b"Hello", f"hex payload decoded to bytes: {seen.payload!r}")

    click("发布格式 JSON")
    app.fill(Query(name="发布内容"), '{"b":2,"a":[1,2]}')
    click("格式化", role="push button")
    formatted = app.read(Query(name="发布内容"))
    check(formatted == json.dumps({"b": 2, "a": [1, 2]}, indent=2), f"formatted JSON: {formatted!r}")


@scenario("T06", "消息列表：搜索、方向过滤、清空")
def t06():
    total = message_count()
    check("/" not in total and int(total) >= 8, f"message count {total}")
    app.fill(Query(name="搜索消息"), "pub/json")
    wait_until(lambda: "/" in message_count(), 4, desc="filtered count")
    shown = messages()
    check(shown and all("pub/json" in m for m in shown), f"search result {shown}")
    app.fill(Query(name="搜索消息"), "")
    wait_until(lambda: message_count() == total, 4, desc="count restored")

    click("消息过滤 发布")
    check(pressed("消息过滤 发布") and not pressed("消息过滤 全部"), "filter toggle state")
    wait_until(lambda: messages() and all(m.startswith("消息 PUB ") for m in messages()), 4, desc="only PUB")
    click("消息过滤 接收")
    wait_until(lambda: messages() and all(m.startswith("消息 RCV ") for m in messages()), 4, desc="only RCV")
    click("消息过滤 全部")

    click("清空消息")
    confirm("清空")
    wait_until(lambda: message_count() == "0", 4, desc="list cleared")
    check(not messages(), "no message rows after clearing")


@scenario("T07", "消息详情：JSON/HEX 视图、复制、复制到发布")
def t07():
    broker.pub("e2e/detail", '{"k":"v","n":[1,2]}')
    row = wait_message(r"^消息 RCV e2e/detail JSON")
    click(row, role="list item")
    app.find(name="消息内容")
    expected = json.dumps({"k": "v", "n": [1, 2], "handled_by": "script"}, indent=2)
    check(pressed("查看方式 JSON"), "JSON view selected for JSON payloads")
    content = app.read(Query(name="消息内容"))
    check(content == expected, f"pretty JSON view: {content!r}")
    click("复制内容", role="push button")
    toast("已复制到剪贴板")
    check(app.clipboard() == expected, f"clipboard: {app.clipboard()!r}")
    click("查看方式 HEX")
    hexdump = app.read(Query(name="消息内容"))
    check(hexdump.startswith("00000000  7B 22 6B 22") and '{"k":"v"' in hexdump, f"hex dump: {hexdump[:120]!r}")
    click("复制到发布", role="push button")
    toast("已复制到发布面板")
    app.wait_gone(role="dialog")
    check(app.read(Query(name="发布 Topic")) == "e2e/detail", "topic copied to publish panel")
    check('"k":"v"' in app.read(Query(name="发布内容")), "payload copied to publish panel")
    check(pressed("发布格式 JSON"), "format copied to publish panel")


@scenario("T08", "预处理脚本：测试运行、发送前处理、语法/运行错误、启停、删除")
def t08():
    open_scripts()
    app.find(role="list item", name="脚本 接收标记 接收后 已启用")

    click("新增", role="push button")
    app.fill(Query(name="脚本名称"), "e2e-sha")
    click("发送前处理", role="radio button")
    app.fill(Query(name="脚本代码"), "async function process(p) {\n  console.log('len', p.length);\n  return await crypto.sha256(p);\n}")
    app.fill(Query(name="测试 payload"), "abc")
    click("测试运行", role="push button")
    out = app.find(role="status bar", pattern=r"^测试结果 ").name
    check("运行成功" in out and hashlib.sha256(b"abc").hexdigest() in out, f"test run output: {out}")
    click("保存", role="push button")
    toast("脚本已保存")
    app.find(role="list item", name="脚本 e2e-sha 发送前 已启用")
    s = find_one(data()["scripts"], name="e2e-sha")
    check(s and s["script_type"] == "before_publish" and s["enabled"], f"persisted script {s}")

    app.fill(Query(name="脚本代码"), "while (true) {}")
    click("测试运行", role="push button")
    out = app.find(role="status bar", pattern=r"^测试结果 运行失败", timeout=30).name
    check("运行限制" in out, f"runaway script stopped: {out}")
    app.fill(Query(name="脚本代码"), "async function process(p) { return await crypto.sha256(p); }")
    click("保存", role="push button")
    toast("脚本已保存")

    click("新增", role="push button")
    app.fill(Query(name="脚本名称"), "e2e-syntax")
    app.fill(Query(name="脚本代码"), "function process( {")
    click("保存", role="push button")
    toast("语法错误")
    check(find_one(data()["scripts"], name="e2e-syntax") is None, "invalid script not saved")

    new_script("e2e-throw", 'function process() { throw new Error("boom-e2e"); }')
    toast("脚本已保存")
    close_dialogs()

    mark = spy.mark()
    publish("e2e/pub/script", "hello", "TEXT", expect_ok=False)
    toast("boom-e2e")
    wait_message(r"^消息 PUB e2e/pub/script TEXT Q0 hello 脚本错误: .*boom-e2e")
    spy.assert_silent(mark, "e2e/pub/script", wait=0.5)
    logs = "".join(p.read_text() for p in (DATA / "logs").glob("*.log"))
    check("boom-e2e" in logs, "script error written to the error log")

    open_scripts()
    click("启用脚本 e2e-throw")
    app.find(role="list item", name="脚本 e2e-throw 发送前 已停用")
    check(find_one(data()["scripts"], name="e2e-throw")["enabled"] is False, "disabled state persisted")
    close_dialogs()
    publish("e2e/pub/script", "hello", "TEXT")
    seen = spy.wait(mark, "e2e/pub/script")[0]
    check(seen.payload == hashlib.sha256(b"hello").hexdigest().encode(), f"before-publish script applied: {seen.payload!r}")

    open_scripts()
    click("启用脚本 e2e-sha")
    app.find(role="list item", name="脚本 e2e-sha 发送前 已停用")
    app.click(Query(role="list item", name="脚本 e2e-throw 发送前 已停用"), dx=0.3)
    check(app.read(Query(name="脚本名称")) == "e2e-throw", "selecting a script loads it into the editor")
    click("删除", role="push button")
    confirm("删除")
    app.wait_gone(role="list item", pattern=r"^脚本 e2e-throw ")
    check(find_one(data()["scripts"], name="e2e-throw") is None, "script deleted from data.yaml")
    click("查看可用函数", role="push button")
    click("知道了", role="push button")
    close_dialogs()
    mark = spy.mark()
    publish("e2e/pub/script", "plain", "TEXT")
    check(spy.wait(mark, "e2e/pub/script")[0].payload == b"plain", "no active before-publish script")


@scenario("T09", "命令模板：保存、快捷命令、抽屉搜索/分类/使用、复制、编辑、删除")
def t09():
    click("发布格式 JSON")
    app.fill(Query(name="发布 Topic"), "e2e/tpl/topic")
    app.fill(Query(name="发布内容"), '{"t":1}')
    click("保存为模板")
    check(app.read(Query(name="模板主题")) == "e2e/tpl/topic", "template dialog prefilled from publish panel")
    click("创建模板", role="push button")
    toast("请输入模板名称")
    app.fill(Query(name="模板名称"), "e2e-tpl")
    app.fill(Query(name="模板分类"), "测试")
    click("模板 QoS QoS 2")
    click("创建模板", role="push button")
    toast("模板已创建")
    app.find(role="push button", name="e2e-tpl")
    t = find_one(data()["templates"], name="e2e-tpl")
    check(t and t["category"] == "测试" and t["qos"] == 2 and t["payload_type"] == "json", f"persisted template {t}")

    app.fill(Query(name="发布 Topic"), "")
    click("e2e-tpl", role="push button")
    toast("已加载: e2e-tpl")
    check(app.read(Query(name="发布 Topic")) == "e2e/tpl/topic", "quick command loads topic")
    check(pressed("发布 QoS QoS 2"), "quick command loads QoS")
    check(find_one(data()["templates"], name="e2e-tpl")["use_count"] == 1, "use count incremented")

    click("管理模板")
    app.find(name="搜索模板")
    app.find(role="list item", pattern=r"^模板 e2e-tpl e2e/tpl/topic 测试 使用 1 次$")
    app.fill(Query(name="搜索模板"), "light")
    wait_until(lambda: [n.name for n in app.find_all(role="list item", pattern="^模板 ")] == ["模板 开灯 e2e/light 照明 使用 3 次"], 4, desc="search filters templates")
    app.fill(Query(name="搜索模板"), "")
    click("测试", role="push button")
    wait_until(lambda: [n.name for n in app.find_all(role="list item", pattern="^模板 ")] == ["模板 e2e-tpl e2e/tpl/topic 测试 使用 1 次"], 4, desc="category filter")
    click("全部", role="push button")
    click("使用模板 开灯")
    toast("已加载: 开灯")
    wait_until(lambda: not app.find_all(name="搜索模板"), 4, desc="drawer closes after use")
    check(app.read(Query(name="发布 Topic")) == "e2e/light", "drawer 'use' loads the template")
    check(find_one(data()["templates"], name="开灯")["use_count"] == 4, "use count incremented from drawer")

    click("管理模板")
    click("模板菜单 e2e-tpl")
    click("复制", role="menu item")
    toast("模板已复制")
    app.find(role="list item", pattern=r"^模板 e2e-tpl - 副本 ")
    click("模板菜单 e2e-tpl - 副本")
    click("编辑", role="menu item")
    app.fill(Query(name="模板名称"), "e2e-tpl-2")
    click("保存", role="push button")
    toast("模板已更新")
    app.find(role="list item", pattern=r"^模板 e2e-tpl-2 ")
    click("模板菜单 e2e-tpl-2")
    click("删除", role="menu item")
    confirm("删除")
    app.wait_gone(role="list item", pattern=r"^模板 e2e-tpl-2 ")
    check(find_one(data()["templates"], name="e2e-tpl-2") is None, "template deleted from data.yaml")
    close_dialogs()


@scenario("T10", "定时发布：按顺序指定次数、HEX 字节、后台运行与停止")
def t10():
    click("定时发布", role="push button")
    click("开始发布", role="push button")
    toast("请至少选择一条命令")
    click("命令 心跳", role="list item")
    click("命令 开灯", role="list item")
    app.find(role="list item", name="命令 心跳 已选 #1")
    app.find(role="list item", name="命令 开灯 已选 #2")
    app.fill(Query(name="发送间隔"), "250")
    click("指定次数", role="radio button")
    app.fill(Query(name="循环次数"), "2")
    mark = spy.mark()
    started = time.time()
    click("开始发布", role="push button")
    app.find(role="status bar", name="定时发布 已结束 轮次 2 已发送 4 成功 4 失败 0", timeout=15)
    toast("定时发布完成")
    seen = [m for m in spy.since(mark) if m.topic in ("e2e/heartbeat", "e2e/light")]
    check([m.topic for m in seen] == ["e2e/heartbeat", "e2e/light"] * 2, f"order: {[m.topic for m in seen]}")
    check(all(m.payload == b"\x01\x02\x03\xff" for m in seen if m.topic == "e2e/heartbeat"), "HEX template sent as bytes")
    check(all(m.qos == 1 for m in seen if m.topic == "e2e/light"), "template QoS honoured")
    gaps = [b.at - a.at for a, b in zip(seen, seen[1:])]
    # The interval is a minimum spacing between consecutive sends (no catch-up bursts).
    check(all(g > 0.2 for g in gaps) and time.time() - started < 10, f"interval respected: {gaps}")

    click("返回配置", role="push button")
    click("无限循环", role="radio button")
    app.fill(Query(name="发送间隔"), "100")
    click("开始发布", role="push button")
    app.find(role="status bar", pattern=r"^定时发布 运行中 ")
    click("后台运行", role="push button")
    app.wait_gone(role="dialog")
    app.find(role="status bar", pattern=r"^定时发布进度 ")
    time.sleep(1.0)
    click("停止", role="push button")
    toast("已停止定时发布")
    app.wait_gone(role="status bar", pattern=r"^定时发布进度 ")
    count = len(spy.since(mark))
    time.sleep(1.0)
    check(len(spy.since(mark)) == count, "no messages after stopping")


@scenario("T11", "服务器管理：WebSocket + MQTT 3.1.1、多连接、编辑、复制、删除")
def t11():
    click("新增服务器")
    click("保存", role="push button")
    toast("请输入名称")
    app.fill(Query(name="服务器名称"), "E2E WS")
    app.fill(Query(name="主机地址"), "127.0.0.1")
    click("传输协议 ws://")
    check(app.read(Query(name="端口")) == "8083", "port follows ws:// default")
    app.fill(Query(name="端口"), str(WS))
    click("MQTT 3.1.1", role="radio button")
    click("保存", role="push button")
    toast("服务器已添加")
    app.find(role="list item", name=f"服务器 E2E WS ws://127.0.0.1:{WS} 未连接 当前")
    s = find_one(data()["servers"], name="E2E WS")
    check(s and s["protocol"] == "ws" and s["port"] == WS and s["protocol_version"] == "3.1.1", f"persisted {s}")

    click("连接")
    status("已连接")
    check(app.exists(role="list item", pattern=r"^服务器 本地 TCP .* 已连接$"), "first server still connected")
    check(not app.find_all(role="list item", pattern=r"^订阅 "), "new server starts without subscriptions")
    click("新增订阅")
    app.fill(Query(name="订阅 Topic"), "ws/#")
    click("订阅", role="push button")
    toast("订阅已添加")
    time.sleep(0.4)
    broker.pub("ws/in", "via-websocket")
    wait_message(r"^消息 RCV ws/in TEXT Q0 via-websocket$")
    mark = spy.mark()
    publish("ws/out", "from-ws-client", "TEXT")
    check(spy.wait(mark, "ws/out")[0].payload == b"from-ws-client", "publish over WebSocket")

    select_server("本地 TCP")
    assert_no_message(r"ws/in", wait=0.3)  # message lists are per server
    select_server("E2E WS")

    click("编辑服务器")
    app.fill(Query(name="服务器名称"), "E2E WS 2")
    click("保存", role="push button")
    toast("服务器已更新")
    toast("重新连接后生效")
    app.find(server_q("E2E WS 2"))

    click("服务器菜单 E2E WS 2")
    click("复制", role="menu item")
    app.find(server_q("E2E WS 2 (副本)"))
    dup = find_one(data()["servers"], name="E2E WS 2 (副本)")
    check(dup and dup["port"] == WS and not dup.get("client_id"), f"duplicate {dup}")
    click("服务器菜单 E2E WS 2 (副本)")
    click("删除", role="menu item")
    confirm("删除")
    app.wait_gone(role="list item", pattern=r"^服务器 E2E WS 2 \(副本\) ")
    check(find_one(data()["servers"], name="E2E WS 2 (副本)") is None, "server deleted from data.yaml")


@scenario("T12", "TLS（mqtts + 自定义 CA）、证书错误与连接失败处理")
def t12():
    click("新增服务器")
    app.fill(Query(name="服务器名称"), "E2E TLS")
    app.fill(Query(name="主机地址"), "localhost")
    click("传输协议 mqtts://")
    check(app.read(Query(name="端口")) == "8883", "port follows mqtts:// default")
    app.fill(Query(name="端口"), str(TLS))
    click("高级", role="page tab")
    app.fill(Query(name="CA 证书"), CA_PEM)
    click("保存", role="push button")
    toast("服务器已添加")
    s = find_one(data()["servers"], name="E2E TLS")
    check(s and s["use_tls"] and "BEGIN CERTIFICATE" in (s.get("ca_cert") or ""), "TLS settings persisted")
    click("连接")
    status("已连接")
    mark = spy.mark()
    publish("tls/out", "over-tls", "TEXT", qos=1)
    check(spy.wait(mark, "tls/out")[0].payload == b"over-tls", "publish over TLS")
    click("断开")
    status("未连接")

    click("编辑服务器")
    click("高级", role="page tab")
    app.fill(Query(name="CA 证书"), "")
    click("保存", role="push button")
    click("连接")
    status("连接错误")
    toast("证书")

    select_server("不可达")
    click("连接")
    err = status("连接错误").name
    check("refused" in err.lower(), f"status carries the raw error: {err}")
    toast("连接被拒绝")
    logs = "".join(p.read_text() for p in (DATA / "logs").glob("*.log"))
    check("Connection refused" in logs and "不可达" in logs, "connection error written to the error log")


@scenario("T13", "设置：主题切换与持久化、数据/日志路径、日志查看与清空")
def t13():
    click("深色模式", role="push button")
    app.find(role="push button", name="浅色模式")
    check(config().get("theme") == "dark", f"config.yaml: {config()}")
    click("系统设置")
    app.find(role="status bar", name=f"数据文件 {DATA / 'data.yaml'}")
    app.find(role="status bar", name=f"日志目录 {DATA / 'logs'}")
    check(app.find(name="深色", role="radio button").checked, "theme radio reflects dark mode")
    recent = app.read(Query(name="最近日志"))
    check("Connection refused" in recent, f"recent log view: {recent[:200]!r}")
    click("浅色", role="radio button")
    wait_until(lambda: config().get("theme") == "light", 4, desc="theme light persisted")
    click("跟随系统", role="radio button")
    wait_until(lambda: config().get("theme") == "system", 4, desc="theme system persisted")
    click("深色", role="radio button")
    wait_until(lambda: config().get("theme") == "dark", 4, desc="theme dark persisted")
    click("清空日志", role="push button")
    confirm("清空")
    toast("日志已清空")
    check(not list((DATA / "logs").glob("*.log")), "log files removed")
    close_dialogs()


@scenario("T16", "环境变量：旧数据保留、变量管理、订阅 / 发布替换与脚本 env")
def t16():
    # Fields the previous version wrote survived every save so far.
    d = data()
    var = find_one(d.get("env_variables") or [], name="DEVICE_ID")
    check(var and var["value"] == "dev-e2e" and var["description"] == "设备编号", f"legacy env variable: {var}")
    sub = find_one(d["subscriptions"], topic="sensor/+/temp")
    check(sub and sub.get("color") == "#F56C6C", f"legacy subscription colour: {sub}")

    select_server("本地 TCP")
    if not app.exists(role="status bar", pattern=r"^连接状态 已连接"):
        click("连接")
    status("已连接")

    click("环境变量", role="push button")
    app.find(role="list item", name="环境变量 DEVICE_ID dev-e2e")
    click("新增变量", role="push button")
    app.fill(Query(name="变量名"), "1bad")
    app.fill(Query(name="变量值"), "x")
    click("保存", role="push button")
    toast("变量名格式不正确")
    check(app.find_all(role="dialog"), "dialog must stay open on an invalid name")
    app.fill(Query(name="变量名"), "SUFFIX")
    app.fill(Query(name="变量值"), "tail")
    click("保存", role="push button")
    toast("变量已保存")
    app.find(role="list item", name="环境变量 SUFFIX tail")
    check(find_one(data()["env_variables"], name="SUFFIX", server_id=1, value="tail"), "variable persisted")
    close_dialogs()

    click("新增订阅")
    app.fill(Query(name="订阅 Topic"), "env/{{DEVICE_ID}}/#")
    click("订阅", role="push button")
    toast("订阅已添加")
    app.find(role="list item", name="订阅 env/dev-e2e/# Q0 已启用")
    check(find_one(data()["subscriptions"], topic="env/dev-e2e/#"), "subscription stored with the variable substituted")
    time.sleep(0.4)

    mark = spy.mark()
    publish("env/{{DEVICE_ID}}/cmd", '{"id":"{{DEVICE_ID}}-{{SUFFIX}}"}', "JSON")
    seen = spy.wait(mark, "env/dev-e2e/cmd")[0]
    check(seen.payload == b'{"id":"dev-e2e-tail"}', f"payload with variables: {seen.payload!r}")
    wait_message(r'^消息 RCV env/dev-e2e/cmd JSON Q0 \{"id":"dev-e2e-tail","handled_by":"script"\}$')

    open_scripts()
    new_script("e2e-env", 'function process(p) {\n  return env.replace("{{DEVICE_ID}}") + ":" + env.get("SUFFIX") + ":" + p;\n}')
    toast("脚本已保存")
    close_dialogs()
    mark = spy.mark()
    publish("env/plain", "x", "TEXT")
    seen = spy.wait(mark, "env/plain")[0]
    check(seen.payload == b"dev-e2e:tail:x", f"script reads the variables: {seen.payload!r}")
    open_scripts()
    app.click(Query(role="list item", name="脚本 e2e-env 发送前 已启用"), dx=0.3)
    click("删除", role="push button")
    confirm("删除")
    app.wait_gone(role="list item", pattern=r"^脚本 e2e-env ")
    close_dialogs()


@scenario("T17", "订阅编辑与颜色标记、界面语言切换")
def t17():
    row = app.find(role="list item", name="订阅 env/dev-e2e/# Q0 已启用")
    app.click(row, dx=0.5)  # hover reveals the edit / delete buttons
    click("编辑订阅 env/dev-e2e/#")
    check(app.read(Query(name="订阅 Topic")) == "env/dev-e2e/#", "edit dialog prefilled with the topic")
    click("订阅 QoS QoS 1")
    click("颜色 #409EFF")
    click("保存", role="push button")
    toast("订阅已更新")
    app.find(role="list item", name="订阅 env/dev-e2e/# Q1 已启用")
    sub = find_one(data()["subscriptions"], topic="env/dev-e2e/#")
    check(sub and sub["qos"] == 1 and sub.get("color") == "#409EFF", f"edited subscription {sub}")
    time.sleep(0.4)
    broker.pub("env/dev-e2e/q1", "after-edit", qos=1)
    wait_message(r"^消息 RCV env/dev-e2e/q1 TEXT Q1 after-edit$")

    click("系统设置")
    click("English", role="radio button")
    app.find(role="push button", name="Settings")
    wait_until(lambda: config().get("language") == "en-US", 4, desc="language en-US persisted")
    click("简体中文", role="radio button")
    app.find(role="push button", name="系统设置")
    wait_until(lambda: config().get("language") == "zh-CN", 4, desc="language zh-CN persisted")
    close_dialogs()


@scenario("T14", "重启应用：数据、主题与脚本持久化")
def t14():
    app.stop()
    app.start()
    for name in ("本地 TCP", "不可达", "E2E WS 2", "E2E TLS"):
        app.find(server_q(name))
    app.find(role="push button", name="浅色模式")  # dark theme restored
    select_server("本地 TCP")
    app.find(role="list item", name="订阅 e2e/# Q0 已启用")
    app.find(role="list item", name="订阅 sensor/+/temp Q1 已启用")
    app.find(role="push button", name="e2e-tpl")
    open_scripts()
    app.find(role="list item", name="脚本 e2e-sha 发送前 已停用")
    app.find(role="list item", name="脚本 接收标记 接收后 已启用")
    close_dialogs()
    status("未连接")
    check(data().get("messages") == [], "live messages are not persisted")
    click("连接")
    status("已连接")
    broker.pub("e2e/after-restart", "ok")
    wait_message(r"^消息 RCV e2e/after-restart TEXT Q0 ok$")


@scenario("T15", "删除服务器：级联删除订阅、模板、脚本与环境变量")
def t15():
    click("服务器菜单 本地 TCP")
    click("删除", role="menu item")
    confirm("删除")
    app.wait_gone(role="list item", pattern=r"^服务器 本地 TCP ")
    d = data()
    for key in ("subscriptions", "templates", "scripts", "env_variables"):
        leftovers = [i for i in d.get(key) or [] if i["server_id"] == 1]
        check(not leftovers, f"{key} of the deleted server remain: {leftovers}")
    app.find(role="list item", pattern=r"^服务器 .* 当前$")  # another server became active


# ---------------------------------------------------------------------------
# Runner


def main() -> int:
    global spy
    seed()
    spy = Spy(broker)
    results = []
    app.start()
    try:
        for test_id, title, fn in TESTS:
            if ONLY and test_id not in ONLY:
                continue
            started = time.time()
            error = None
            try:
                if not app.alive():
                    raise E2EError("application is not running")
                fn()
            except Exception as err:  # record and continue with the next scenario
                error = "".join(traceback.format_exception_only(type(err), err)).strip()
                detail = traceback.format_exc(limit=4)
                (WORK / f"{test_id}-failure.txt").write_text(f"{detail}\n\n--- tree ---\n{app.dump() if app.alive() else ''}")
            shot = app.screenshot(f"{test_id}-{'pass' if error is None else 'fail'}") if app.alive() else None
            results.append(dict(id=test_id, title=title, ok=error is None, seconds=round(time.time() - started, 1),
                                error=error, screenshot=shot.name if shot else None))
            mark = "PASS" if error is None else "FAIL"
            print(f"[{mark}] {test_id} {title} ({results[-1]['seconds']}s)", flush=True)
            if error:
                print("       " + error.splitlines()[0][:300], flush=True)
                close_dialogs() if app.alive() else None
    finally:
        try:
            app.stop()
        except Exception:
            pass
        spy.stop()

    passed = sum(r["ok"] for r in results)
    (WORK / "report.json").write_text(json.dumps(results, ensure_ascii=False, indent=2))
    lines = [
        "# Mini MQTT Client E2E report",
        "",
        f"Binary: `{os.environ['E2E_BIN']}`  ",
        f"Broker: mosquitto (TCP {TCP}, WebSocket {WS}, TLS {TLS})  ",
        f"Result: **{passed}/{len(results)} passed**",
        "",
        "| ID | Scenario | Result | Time | Screenshot |",
        "|----|----------|--------|------|------------|",
    ]
    for r in results:
        lines.append(f"| {r['id']} | {r['title']} | {'✅' if r['ok'] else '❌ ' + (r['error'] or '').splitlines()[0][:120]} "
                     f"| {r['seconds']}s | {r['screenshot'] or ''} |")
    (WORK / "report.md").write_text("\n".join(lines) + "\n")
    print(f"\n{passed}/{len(results)} scenarios passed")
    return 0 if passed == len(results) and results else 1


if __name__ == "__main__":
    sys.exit(main())
