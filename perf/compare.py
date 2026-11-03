#!/usr/bin/env python3
"""Print before/after Markdown tables from two perf result directories.

    python3 perf/compare.py <results-before> <results-after>
"""
import json
import sys
from pathlib import Path

BEFORE, AFTER = Path(sys.argv[1]), Path(sys.argv[2])


def load(d: Path, name: str):
    p = d / f"{name}.json"
    return json.loads(p.read_text()) if p.exists() else None


def pmax(probes):
    vals = [v for p in probes or [] for k, v in p.items() if k != "at_s" and v is not None]
    return max(vals) if vals else None


def fmt(v, nd=1):
    if v is None:
        return "—"
    if isinstance(v, float):
        return f"{v:,.{nd}f}"
    if isinstance(v, int):
        return f"{v:,}"
    return str(v)


def step_row(s, label):
    if not s:
        return f"| {label} | — |"
    L = s.get("load", {})
    pe = s.get("probe_at_load_end") or {}
    pe = max([v for v in pe.values() if v is not None], default=None)
    return ("| {} | {} | {} | {} | {} | {} / {} | {} | {} | {} | {} | {} | {} | {} → {} |".format(
        label, fmt(s.get("sent")), fmt(s.get("dropped", 0)), fmt(s.get("app_throughput")), fmt(s.get("display_rate")),
        fmt(s.get("lag_p50_s"), 2), fmt(s.get("lag_max_s"), 2), fmt(pmax(s.get("probes_during")), 3), fmt(pe, 3),
        fmt(s.get("drain_s"), 2), fmt(L.get("cpu_avg")), fmt(L.get("th_main_avg")), fmt(L.get("th_llvmpipe_avg")),
        fmt(s.get("rss_before")), fmt(s.get("rss_after_drain"))))


HDR = ("| 档位 | 发送 | 丢弃 | 进入应用吞吐 | 显示速率 | 显示延迟 p50 / max s | 负载中点击最大 s | 负载结束时点击 s | 排空 s "
       "| 应用 CPU% | 主线程% | llvmpipe% | RSS MB |\n|" + "---|" * 13)


def exp1():
    b, a = load(BEFORE, "exp1"), load(AFTER, "exp1")
    print("### exp1\n\n" + HDR)
    bs = {s["phase"]: s for s in (b or {}).get("steps", [])}
    as_ = {s["phase"]: s for s in (a or {}).get("steps", [])}
    for ph in ["r100", "r1000", "r5000", "r10000", "r20000", "r40000", "search-r1000", "search-r5000", "max"]:
        if ph in bs:
            print(step_row(bs[ph], f"{ph} 修复前"))
        if ph in as_:
            print(step_row(as_[ph], f"{ph} 修复后"))
    for tag, d in (("修复前", b), ("修复后", a)):
        if d:
            print(f"\n{tag}: 负载后 RSS {d.get('after_load_rss', [[0, None]])[-1][1]} MB，清空后 {d.get('after_clear_rss', [[0, None]])[-1][1]} MB")


def exp2():
    b, a = load(BEFORE, "exp2"), load(AFTER, "exp2b")
    print("\n### exp2 (script)\n")
    if b:
        c = b["capacity"]
        print(f"修复前 突发 {c.get('sent')} 条: 排空 {c.get('drain_s')} s, 吞吐 {c.get('app_throughput')} msg/s")
        for s in b["steps"]:
            print(f"修复前 {s.get('phase')}: backlog_end {s.get('backlog_at_load_end')} lag_max {s.get('lag_max_s')} "
                  f"disp {s.get('display_rate')} drain_rate {s.get('drain_rate')} est {s.get('est_full_drain_s')} "
                  f"rss {s.get('rss_before')}→{s.get('rss_end_load')} probe {pmax(s.get('probes_during'))} "
                  f"main {s.get('load', {}).get('th_main_avg')} worker {s.get('load', {}).get('th_Worker_avg')}")
    if a:
        print("\n" + HDR)
        print(step_row(a.get("capacity"), "burst 修复后"))
        for s in a["steps"]:
            print(step_row(s, f"{s.get('phase')} 修复后"))
        for s in [a.get("capacity", {})] + a["steps"]:
            L = s.get("load", {})
            print(f"  {s.get('phase')}: Worker {L.get('th_Worker_avg')}%  toasts {s.get('drop_toasts_seen')} "
                  f"drop_log_lines {s.get('drop_log_lines')} last_reached {s.get('last_reached')}")


def exp3():
    b, a = load(BEFORE, "exp3"), load(AFTER, "exp3")
    print("\n### exp3\n\n| 大小 | 版本 | RSS 加载前→后 | 计数 | 加载中点击 | 打开详情 | HEX | 关闭 | 搜索生效 | 搜索中点击 | 搜索时主线程 | 清空后 RSS |\n|" + "---|" * 12)
    for tag, d in (("修复前", b), ("修复后", a)):
        for s in (d or {}).get("sizes", []):
            sp = s.get("search_incoming_probe")
            print("| {} | {} | {} → {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |".format(
                s["phase"], tag, s.get("rss_before"), s.get("rss_after_drain"), s.get("count_label"),
                fmt(pmax(s.get("probes_during")), 3), s.get("detail_open_s"), s.get("detail_hex_s"),
                s.get("detail_close_s"), s.get("search_apply_s"), fmt(pmax([sp]) if sp else None, 3),
                (s.get("search_cpu") or {}).get("th_main_avg"), (s.get("rss_after_clear") or [[0, None]])[-1][1]))


def simple(name, keys):
    b, a = load(BEFORE, name), load(AFTER, name)
    print(f"\n### {name}\n")
    for k in keys:
        print(f"- {k}: 修复前 {json.dumps((b or {}).get(k), ensure_ascii=False)} | 修复后 {json.dumps((a or {}).get(k), ensure_ascii=False)}")


exp1()
exp2()
exp3()
simple("exp3c", ["rss_loaded", "hex0_s", "rss_hex_open0", "rss_closed0", "hex1_s", "rss_hex_open1", "rss_after_clear"])
simple("exp4", ["startup", "rss_start", "connect_each_s", "rss_connected", "rss_after_fanout", "switch_server_s"])
simple("exp4b", ["rss_start", "connect_series", "rss_all_connected", "rss_all_disconnected", "rss_reconnected"])
simple("exp6", ["rate_at_broker", "run_s", "progress_label", "stop_s", "last_msg_after_stop_click_ms", "rss_end"])
b, a = load(BEFORE, "exp6"), load(AFTER, "exp6")
for tag, d in (("修复前", b), ("修复后", a)):
    if d:
        br = d.get("broker", {})
        print(f"- {tag} 间隔 p50/p99/max ms: {br.get('gap_ms_p50')}/{br.get('gap_ms_p99')}/{br.get('gap_ms_max')}  "
              f"CPU bg {d.get('background', {}).get('cpu_avg')} main {d.get('background', {}).get('th_main_avg')}")
