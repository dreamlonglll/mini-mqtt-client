#!/usr/bin/env python3
"""Stress / performance experiments for Mini MQTT Client. Run through perf/env.sh:

    PERF_EXP=1 perf/env.sh perf/perf_run.py      # one experiment
    PERF_EXP=1,2,3 perf/env.sh perf/perf_run.py  # several

Results: $PERF_OUT/expN.json (+ samples-*.jsonl, lag-*.jsonl, loadgen logs, app logs, screenshots).
"""

from __future__ import annotations

import json
import os
import re
import statistics
import time
import traceback

import perflib as P
from perflib import (LagWatcher, LoadGen, PerfApp, Query, QuietBroker, Sampler, log, parse_seq, save, server, sub,
                     template, script, wait_seq, wait_seq_stable, drop_log, wait_until, write_data, OUT, WORK,
                     TAG_SCRIPT)

EXPS = [e.strip() for e in os.environ.get("PERF_EXP", "1").split(",") if e.strip()]
broker = QuietBroker()


# ---------------------------------------------------------------------------
# helpers


class Env:
    """A running app + its samplers."""

    def __init__(self, tag: str, sample_interval=1.0, lag_interval=0.5, app_env=None, **data):
        self.tag = tag
        self.data_dir = WORK / f"data-{tag}"
        if self.data_dir.exists():
            import shutil
            shutil.rmtree(self.data_dir)
        self.data_path = write_data(self.data_dir, **data)
        self.app = PerfApp(self.data_dir, tag, app_env)
        self.startup = self.app.timed_start()
        log(tag, "startup", self.startup)
        self.sampler = Sampler(self.app.proc.pid, OUT / f"samples-{tag}.jsonl", sample_interval, broker)
        self.sampler.start()
        self.lag = LagWatcher(self.app, lag_interval)
        self.lag.start()

    def phase(self, name: str):
        self.sampler.phase = name
        self.lag.phase = name

    def rss(self) -> float:
        return round(P._status(self.app.proc.pid).get("VmRSS", 0) / 1024, 1)

    def close(self):
        self.lag.stop()
        self.sampler.stop()
        with open(OUT / f"lag-{self.tag}.jsonl", "w") as f:
            for s in self.lag.samples:
                f.write(json.dumps(s) + "\n")
        issues = P.app_log_issues(self.app)
        alive = self.app.alive()
        try:
            self.app.screenshot(f"{self.tag}-end")
        except Exception:
            pass
        try:
            self.app.stop()
        except Exception:
            if self.app.proc and self.app.proc.poll() is None:
                self.app.proc.kill()
        return dict(alive_at_end=alive, log_issues=issues)


def lag_stats(env: Env, phase: str, seq_min: int = 0) -> dict:
    s = [x for x in env.lag.samples if x["phase"] == phase and x.get("seq") is not None and x["seq"] >= seq_min]
    if len(s) < 2:
        return dict(lag_samples=len(s))
    lags = sorted(x["lag_s"] for x in s)
    span = s[-1]["t"] - s[0]["t"]
    # longest period without the displayed seq changing (UI not updating)
    stall, start = 0.0, s[0]
    for a, b in zip(s, s[1:]):
        if b["seq"] != a["seq"]:
            start = b
        stall = max(stall, b["t"] - start["t"])
    return dict(lag_samples=len(s), lag_p50_s=lags[len(lags) // 2], lag_max_s=lags[-1], lag_last_s=s[-1]["lag_s"],
                display_rate=round((s[-1]["seq"] - s[0]["seq"]) / span, 1) if span > 0 else None,
                longest_display_stall_s=round(stall, 2),
                probe_ms_p50=round(statistics.median(x["probe_s"] for x in s) * 1000, 1))


def run_step(env: Env, phase: str, *, rate: float, duration: float = 0, count: int = 0, seq_start: int = 0,
             size: int = 64, kind: str = "json", topic: str = "perf/a", probe_every: float = 6.0,
             probe_timeout: float = 60.0, drain_timeout: float = 180.0, rss_abort_mb: float = 6000, **extra) -> dict:
    env.phase(phase)
    rss0 = env.rss()
    drops0, drop_lines0 = drop_log(env.data_dir)
    rss_peak, aborted = rss0, False
    lg = LoadGen(topic=topic, rate=rate, duration=duration, count=count, seq_start=seq_start, size=size, kind=kind,
                 log_name=f"loadgen-{env.tag}-{phase}.jsonl", **extra)
    t_start = time.time()
    probes = []
    next_probe = t_start + min(probe_every, max(duration / 2, 1) if duration else probe_every)
    while lg.proc.poll() is None:
        time.sleep(0.2)
        rss_peak = max(rss_peak, env.rss())
        if rss_peak > rss_abort_mb and not aborted:
            log(phase, f"RSS {rss_peak} MB > {rss_abort_mb} MB: aborting the load")
            aborted = True
            lg.stop()
        if time.time() >= next_probe and lg.proc.poll() is None:
            p = env.app.filter_probe(timeout=probe_timeout)
            p["at_s"] = round(time.time() - t_start, 1)
            probes.append(p)
            next_probe = time.time() + probe_every
    fin = lg.wait()
    t_end = time.time()
    rss_end_load = env.rss()
    env.phase(phase + "-drain")
    last = fin.get("seq_last", seq_start)
    # UI responsiveness right when the load stops (while a backlog, if any, drains)
    probe_end = env.app.filter_probe(timeout=drain_timeout)
    t_seen, seq_seen, reached = wait_seq_stable(env.app, last, drain_timeout)
    post_probe = env.app.filter_probe(timeout=probe_timeout)
    time.sleep(0.5)
    drops1, drop_lines1 = drop_log(env.data_dir)
    dropped = drops1 - drops0
    sent = fin.get("sent") or 0
    toasts = [t["name"] for t in env.lag.toasts if t["phase"] in (phase, phase + "-drain")]
    res = dict(
        phase=phase, offered_rate=rate, size=size, kind=kind, sent=sent, send_rate=fin.get("rate"),
        send_s=fin.get("elapsed"), aborted_for_rss=aborted, rss_peak_load=rss_peak,
        dropped=dropped, drop_log_lines=drop_lines1 - drop_lines0, processed=sent - dropped,
        last_seq_shown=seq_seen, last_seq_sent=last, last_reached=reached,
        drop_toasts_seen=sorted(set(toasts))[:6], drop_toast_samples=len(toasts),
        drain_s=round(t_seen - t_end, 2) if t_seen else f">{drain_timeout}",
        app_throughput=round((sent - dropped) / (t_seen - t_start), 1) if t_seen and sent else None,
        rss_before=rss0, rss_end_load=rss_end_load, rss_after_drain=env.rss(),
        probes_during=probes, probe_at_load_end=probe_end, probe_after=post_probe,
        **lag_stats(env, phase, seq_start),
        load=env.sampler.summary(phase), drain=env.sampler.summary(phase + "-drain"),
    )
    log(phase, json.dumps({k: v for k, v in res.items() if k not in ("load", "drain")}, ensure_ascii=False))
    log(phase, "load", res["load"])
    return res


def idle_probe(env: Env, seconds=5, n=3) -> dict:
    env.phase("idle")
    time.sleep(seconds)
    probes = [env.app.filter_probe() for _ in range(n)]
    return dict(probes=probes, sampler=env.sampler.summary("idle"), rss=env.rss())


def single_server(**extra):
    return dict(servers=[server(1, "本地")], subscriptions=[sub(1, 1, "perf/#")], **extra)


def clear_messages(env: Env) -> float:
    t0 = time.time()
    env.app.click(Query(role="push button", name="清空消息"))
    env.app.click(Query(role="push button", name="清空", index=-1))
    wait_until(lambda: env.app.message_count() == "0", 60, desc="messages cleared")
    return round(time.time() - t0, 2)


def watch_rss(env: Env, phase: str, seconds: float) -> list:
    env.phase(phase)
    out = []
    t0 = time.time()
    while time.time() - t0 < seconds:
        out.append((round(time.time() - t0, 1), env.rss()))
        time.sleep(2)
    return out


# ---------------------------------------------------------------------------
# 1. inbound throughput without scripts


def exp1():
    env = Env("exp1", **single_server())
    res = dict(startup=env.startup)
    try:
        env.app.connect_and_wait()
        res["idle"] = idle_probe(env)
        steps = []
        base = 0
        for rate, dur in [(100, 20), (1000, 20), (5000, 20), (10000, 20), (20000, 20), (40000, 20)]:
            steps.append(run_step(env, f"r{rate}", rate=rate, duration=dur, seq_start=base))
            base += 10_000_000
        # Same load with a search keyword active ("seq" matches every payload).
        env.app.fill(Query(name="搜索消息"), "seq")
        time.sleep(1)
        for rate in (1000, 5000):
            steps.append(run_step(env, f"search-r{rate}", rate=rate, duration=15, seq_start=base))
            base += 10_000_000
        env.app.fill(Query(name="搜索消息"), "")
        time.sleep(1)
        # Saturation: 1M messages as fast as the broker delivers them.
        steps.append(run_step(env, "max", rate=0, count=1_000_000, seq_start=base, probe_every=8,
                              probe_timeout=180, drain_timeout=400))
        res["steps"] = steps
        res["after_load_rss"] = watch_rss(env, "after", 30)
        res["clear_s"] = clear_messages(env)
        res["after_clear_rss"] = watch_rss(env, "cleared", 16)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp1", res)


# ---------------------------------------------------------------------------
# 2. inbound with an after_receive script


def exp2():
    res = dict(steps=[])
    scr = [script(1, 1, "接收标记", TAG_SCRIPT)]
    # capacity: a burst processed as fast as possible
    env = Env("exp2-cap", **single_server(scripts=scr))
    try:
        env.app.connect_and_wait()
        res["idle"] = idle_probe(env, seconds=3, n=2)
        res["capacity"] = run_step(env, "burst", rate=0, count=3000, drain_timeout=300, probe_every=3)
    except Exception:
        res["error_cap"] = traceback.format_exc()
        log(res["error_cap"])
    close = env.close()
    res.setdefault("capacity", {})["close"] = close
    for rate, dur in [(100, 20), (1000, 15), (5000, 15), (20000, 10)]:
        env = Env(f"exp2-r{rate}", **single_server(scripts=scr))
        step = {}
        try:
            env.app.connect_and_wait()
            phase = f"r{rate}"
            env.phase(phase)
            rss0 = env.rss()
            lg = LoadGen(topic="perf/a", rate=rate, duration=dur, log_name=f"loadgen-exp2-{phase}.jsonl")
            t0 = time.time()
            probes = []
            while lg.proc.poll() is None:
                time.sleep(4)
                if lg.proc.poll() is None:
                    p = env.app.filter_probe(timeout=60)
                    p["at_s"] = round(time.time() - t0, 1)
                    probes.append(p)
            fin = lg.wait()
            t_end = time.time()
            rss_end = env.rss()
            seq_end = env.lag.last_seq() or 0
            env.phase(phase + "-drain")
            t_seen = wait_seq(env.app, fin["seq_last"], 25)
            seq_after = env.lag.last_seq() or 0
            p_after = env.app.filter_probe(timeout=60)
            drain_window = time.time() - t_end
            backlog = fin["seq_last"] - seq_after
            drain_rate = (seq_after - seq_end) / drain_window if drain_window > 0 else 0
            step = dict(
                phase=phase, offered_rate=rate, sent=fin["sent"], send_s=fin["elapsed"],
                displayed_seq_at_load_end=seq_end, backlog_at_load_end=fin["seq_last"] - seq_end,
                drained=bool(t_seen), drain_s=round(t_seen - t_end, 2) if t_seen else None,
                backlog_after_25s=backlog if not t_seen else 0, drain_rate=round(drain_rate, 1),
                est_full_drain_s=round(backlog / drain_rate, 1) if (not t_seen and drain_rate > 0) else None,
                rss_before=rss0, rss_end_load=rss_end, rss_after=env.rss(),
                rss_per_backlog_msg_bytes=round((rss_end - rss0) * 1048576 / max(fin["seq_last"] - seq_end, 1)),
                probes_during=probes, probe_after=p_after,
                **lag_stats(env, phase), load=env.sampler.summary(phase), drain=env.sampler.summary(phase + "-drain"))
            log(phase, json.dumps({k: v for k, v in step.items() if k not in ("load", "drain")}, ensure_ascii=False))
            log(phase, "load", step["load"])
        except Exception:
            step["error"] = traceback.format_exc()
            log(step["error"])
        step["close"] = env.close()
        res["steps"].append(step)
    save("exp2", res)


def exp2b():
    """Script path with the batched pump: same ramp, measured with run_step (drop-aware)."""
    res = dict(steps=[])
    scr = [script(1, 1, "接收标记", TAG_SCRIPT)]
    env = Env("exp2b-cap", **single_server(scripts=scr))
    try:
        env.app.connect_and_wait()
        res["idle"] = idle_probe(env, seconds=3, n=2)
        res["capacity"] = run_step(env, "burst", rate=0, count=20000, drain_timeout=300, probe_every=3)
    except Exception:
        res["error_cap"] = traceback.format_exc()
        log(res["error_cap"])
    close = env.close()
    res.setdefault("capacity", {})["close"] = close
    for rate, dur in [(100, 20), (1000, 15), (5000, 15), (20000, 10)]:
        env = Env(f"exp2b-r{rate}", **single_server(scripts=scr))
        step = {}
        try:
            env.app.connect_and_wait()
            step = run_step(env, f"r{rate}", rate=rate, duration=dur, probe_every=4, drain_timeout=180)
        except Exception:
            step["error"] = traceback.format_exc()
            log(step["error"])
        step["close"] = env.close()
        res["steps"].append(step)
    save("exp2b", res)


# ---------------------------------------------------------------------------
# 3. large payloads


def timed(fn, timeout=180):
    t0 = time.time()
    try:
        fn()
        return round(time.time() - t0, 2)
    except Exception as err:
        return f"failed after {round(time.time() - t0, 1)}s: {str(err).splitlines()[0][:200]}"


def exp3():
    env = Env("exp3", sample_interval=1.0, lag_interval=1.0, **single_server())
    res = dict(startup=env.startup, sizes=[])
    app = env.app
    try:
        app.connect_and_wait()
        res["idle"] = idle_probe(env, seconds=3, n=2)
        base = 0
        for size, rate, count in [(100_000, 100, 1200), (1_000_000, 25, 1100), (5_000_000, 5, 300)]:
            label = f"{size // 1000}KB"
            step = run_step(env, label, rate=rate, count=count, size=size, seq_start=base, probe_every=8,
                            probe_timeout=120, drain_timeout=300)
            base += 10_000_000
            step["count_label"] = app.message_count()
            time.sleep(3)
            step["rss_settled"] = env.rss()
            # message detail dialog of the newest (large) message
            env.phase(label + "-detail")
            rows = [n for n in app.nodes() if n.role == "list item" and n.name.startswith("消息 ")]
            t0 = time.time()
            app.raw_click(rows[0])
            try:
                app.find(name="消息内容", timeout=180)
                step["detail_open_s"] = round(time.time() - t0, 2)
                time.sleep(0.5)
                step["detail_hex_s"] = timed(lambda: (app.raw_click(app.find(name="查看方式 HEX")),
                                                      wait_until(lambda: app.find(name="查看方式 HEX").checked, 180,
                                                                 desc="hex view")))
                t1 = time.time()
                app.key("Escape", settle=0)
                wait_until(lambda: not app.find_all(role="dialog"), 180, desc="dialog closed")
                step["detail_close_s"] = round(time.time() - t1, 2)
            except Exception as err:
                step["detail_error"] = str(err).splitlines()[0][:300]
                app.escape_all()
            step["detail_cpu"] = env.sampler.summary(label + "-detail")
            if size <= 1_000_000:
                # search over the full list: "zzz" matches nothing -> label "0/N"
                env.phase(label + "-search")
                t0 = time.time()
                app.fill(Query(name="搜索消息"), "zzz")
                try:
                    wait_until(lambda: (app.message_count() or "").startswith("0/"), 300, interval=0.05,
                               desc="search applied")
                    step["search_apply_s"] = round(time.time() - t0, 2)
                except Exception as err:
                    step["search_apply_s"] = f"timeout: {err}"
                # incoming messages while a search is active
                lg = LoadGen(topic="perf/a", rate=5, count=20, size=size, seq_start=base,
                             log_name=f"loadgen-exp3-{label}-search.jsonl")
                lg.wait()
                time.sleep(2)
                step["search_incoming_probe"] = app.filter_probe(timeout=120)
                step["search_cpu"] = env.sampler.summary(label + "-search")
                base += 10_000_000
                t0 = time.time()
                app.fill(Query(name="搜索消息"), "")
                try:
                    wait_until(lambda: "/" not in (app.message_count() or "/"), 300, interval=0.05,
                               desc="search cleared")
                    step["search_clear_s"] = round(time.time() - t0, 2)
                except Exception as err:
                    step["search_clear_s"] = f"timeout: {err}"
            env.phase(label + "-clear")
            step["clear_s"] = clear_messages(env)
            step["rss_after_clear"] = watch_rss(env, label + "-cleared", 10)
            log(label, json.dumps({k: v for k, v in step.items() if k not in ("load", "drain")}, ensure_ascii=False))
            res["sizes"].append(step)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp3", res)


def exp3b():
    """Is the memory kept after clearing large messages a leak or allocator retention?"""
    res = {}
    for label, app_env in [("default", None), ("mmap_threshold", {"GLIBC_TUNABLES": "glibc.malloc.mmap_threshold=131072"})]:
        env = Env(f"exp3b-{label}", app_env=app_env, **single_server())
        r = {}
        try:
            env.app.connect_and_wait()
            r["rss_connected"] = env.rss()
            step = run_step(env, "1000KB", rate=50, count=1100, size=1_000_000, probe_every=10, drain_timeout=120)
            r.update(rss_before=step["rss_before"], rss_full=step["rss_after_drain"], main_cpu=step["load"].get("th_main_avg"))
            time.sleep(2)
            r["clear_s"] = clear_messages(env)
            r["rss_after_clear"] = watch_rss(env, "cleared", 8)
            # refill: is the retained memory reused?
            step = run_step(env, "refill", rate=50, count=1100, size=1_000_000, seq_start=10_000_000, probe_every=10,
                            drain_timeout=120)
            r["rss_after_refill"] = step["rss_after_drain"]
        except Exception:
            r["error"] = traceback.format_exc()
            log(r["error"])
        r["close"] = env.close()
        log(label, r)
        res[label] = r
    save("exp3b", res)


def exp3c():
    """Detail dialog (HEX view) of a 1 MB message: is its memory released after closing + clearing?
    Large allocations are forced to mmap so freed memory always returns to the OS."""
    tun = {"GLIBC_TUNABLES": "glibc.malloc.mmap_threshold=131072"}
    env = Env("exp3c", app_env=tun, **single_server())
    app = env.app
    r = {}
    try:
        app.connect_and_wait()
        step = run_step(env, "1000KB", rate=50, count=200, size=1_000_000, probe_every=10, drain_timeout=120)
        r["rss_loaded"] = step["rss_after_drain"]
        for i in range(3):
            rows = [n for n in app.nodes() if n.role == "list item" and n.name.startswith("消息 ")]
            t0 = time.time()
            app.raw_click(rows[0])
            app.find(name="消息内容", timeout=120)
            r[f"open{i}_s"] = round(time.time() - t0, 2)
            r[f"hex{i}_s"] = timed(lambda: (app.raw_click(app.find(name="查看方式 HEX")),
                                            wait_until(lambda: app.find(name="查看方式 HEX").checked, 120, desc="hex")))
            time.sleep(1)
            r[f"rss_hex_open{i}"] = env.rss()
            app.key("Escape", settle=0)
            wait_until(lambda: not app.find_all(role="dialog"), 60, desc="dialog closed")
            time.sleep(3)
            r[f"rss_closed{i}"] = env.rss()
        r["clear_s"] = clear_messages(env)
        r["rss_after_clear"] = watch_rss(env, "cleared", 10)
    except Exception:
        r["error"] = traceback.format_exc()
        log(r["error"])
    r["close"] = env.close()
    log("exp3c", r)
    save("exp3c", r)


# ---------------------------------------------------------------------------
# 4. many connections / subscriptions


def ensure_visible(app: PerfApp, query: Query, top=70, bottom=740, x=130):
    for _ in range(300):
        n = app.find(query)
        _, y, _, h = n.extents()
        if y >= top and y + h <= bottom:
            return n
        ox, oy = app.window_origin()
        app.xdo("mousemove", str(ox + x), str(oy + 400), "click", "5" if y + h > bottom else "4")
        time.sleep(0.03)
    raise P.E2EError(f"could not scroll {query} into view")


def select_server(app: PerfApp, name: str) -> float:
    q = Query(role="list item", pattern=rf"^服务器 {re.escape(name)} ")
    node = ensure_visible(app, q)
    t0 = time.time()
    app.raw_click(node, dx=0.3)
    app.find(role="list item", pattern=rf"^服务器 {re.escape(name)} .* 当前$", timeout=60)
    return time.time() - t0


def exp4(n_servers=30, subs_per_server=20):
    servers = [server(i) for i in range(1, n_servers + 1)]
    subs = []
    for s in servers:
        i = s["id"]
        subs.append(sub(len(subs) + 1, i, "perf/all/#"))
        subs.extend(sub(len(subs) + 1, i, f"perf/s{i}/t{j}/#") for j in range(1, subs_per_server))
    env = Env("exp4", servers=servers, subscriptions=subs)
    app = env.app
    res = dict(servers=n_servers, subscriptions=len(subs), startup=env.startup, rss_start=env.rss())
    try:
        env.phase("connecting")
        times = []
        t_all = time.time()
        for s in servers:
            select_server(app, s["name"])
            t0 = time.time()
            app.raw_click(app.find(role="push button", name="连接"))
            app.find(role="status bar", pattern=r"^连接状态 已连接", timeout=30)
            times.append(round(time.time() - t0, 3))
        res["connect_total_s"] = round(time.time() - t_all, 1)
        res["connect_each_s"] = dict(min=min(times), median=statistics.median(times), max=max(times))
        time.sleep(3)
        res["app_broker_sockets"] = P.broker_sockets(app.proc.pid)
        res["rss_connected"] = env.rss()
        res["idle"] = idle_probe(env, seconds=8, n=2)
        # fan-out: one publish -> every connection
        res["fanout"] = run_step(env, "fanout", rate=200, duration=20, topic="perf/all/x", probe_every=6,
                                 probe_timeout=120, drain_timeout=300)
        res["fanout"]["app_inbound_rate"] = 200 * n_servers
        res["rss_after_fanout"] = env.rss()
        env.phase("switch")
        sw = []
        for name in ("S001", "S015", "S030"):
            t = select_server(app, name)
            wait_until(lambda: app.message_count() == "1000", 60, desc="count of switched server")
            sw.append((name, round(t, 3)))
        res["switch_server_s"] = sw
        res["after_rss"] = watch_rss(env, "after", 10)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp4", res)


def exp4b(n_servers=30, subs_per_server=20):
    """Attribute the RSS growth seen while connecting many servers: selecting vs. connecting vs. disconnecting."""
    servers = [server(i) for i in range(1, n_servers + 1)]
    subs = []
    for s_ in servers:
        i = s_["id"]
        subs.append(sub(len(subs) + 1, i, "perf/all/#"))
        subs.extend(sub(len(subs) + 1, i, f"perf/s{i}/t{j}/#") for j in range(1, subs_per_server))
    env = Env("exp4b", servers=servers, subscriptions=subs)
    app = env.app
    res = dict(rss_start=env.rss())
    try:
        env.phase("select-only")
        for s_ in servers:
            select_server(app, s_["name"])
        time.sleep(2)
        res["rss_after_select_all"] = env.rss()
        env.phase("select-only-2")
        for s_ in servers:
            select_server(app, s_["name"])
        time.sleep(2)
        res["rss_after_select_all_again"] = env.rss()
        env.phase("connect")
        series = []
        for k, s_ in enumerate(servers):
            select_server(app, s_["name"])
            app.raw_click(app.find(role="push button", name="连接"))
            app.find(role="status bar", pattern=r"^连接状态 已连接", timeout=30)
            if k % 5 == 4:
                time.sleep(1)
                series.append((k + 1, env.rss(), P.broker_sockets(app.proc.pid)))
        res["connect_series"] = series
        time.sleep(3)
        res["rss_all_connected"] = env.rss()
        env.phase("disconnect")
        for s_ in servers:
            select_server(app, s_["name"])
            app.raw_click(app.find(role="push button", name="断开"))
            app.find(role="status bar", pattern=r"^连接状态 未连接", timeout=30)
        time.sleep(3)
        res["rss_all_disconnected"] = env.rss()
        res["sockets_after_disconnect"] = P.broker_sockets(app.proc.pid)
        env.phase("reconnect")
        for s_ in servers:
            select_server(app, s_["name"])
            app.raw_click(app.find(role="push button", name="连接"))
            app.find(role="status bar", pattern=r"^连接状态 已连接", timeout=30)
        time.sleep(3)
        res["rss_reconnected"] = env.rss()
        res["sockets_reconnected"] = P.broker_sockets(app.proc.pid)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    log("exp4b", res)
    save("exp4b", res)


# ---------------------------------------------------------------------------
# 5. big data file


def exp5(n_servers=100, n_templates=2000, n_legacy=10_000):
    servers = [server(1, "本地")] + [server(i) for i in range(2, n_servers + 1)]
    subs = [sub(1, 1, "perf/#")]
    for s in servers[1:]:
        subs.extend(sub(len(subs) + 1, s["id"], f"dev/{s['id']}/t{j}/#", active=j % 2 == 0) for j in range(5))
    tpls = [template(i, 1, f"tpl-{i:04d}", f"perf/tpl/{i}", json.dumps({"cmd": "set", "id": i, "value": i * 3}),
                     category=f"cat{i % 20}") for i in range(1, n_templates + 1)]
    msgs = [dict(id=i, server_id=(i % n_servers) + 1, direction="received", topic=f"legacy/{i % 50}",
                 payload=json.dumps({"seq": i, "temp": 20 + i % 10, "note": "legacy history entry " * 4}),
                 payload_format="json", qos=0, retain=False, created_at=P.NOW) for i in range(1, n_legacy + 1)]
    scr = [script(1, 2, "s", TAG_SCRIPT)]
    env = Env("exp5", servers=servers, subscriptions=subs, templates=tpls, messages=msgs, scripts=scr)
    app = env.app
    path = env.data_path
    res = dict(file_mb=round(path.stat().st_size / 1048576, 2), startup=env.startup, rss_start=env.rss())
    log("exp5 file", res["file_mb"], "MB")
    try:
        res["idle"] = idle_probe(env, seconds=4, n=2)
        # toggle the subscription switch: UI update + rewrite of data.yaml
        env.phase("toggle")
        toggles = []
        for i in range(6):
            node = ensure_visible(app, Query(name="启用订阅 perf/#"))
            want = "已停用" if i % 2 == 0 else "已启用"
            m0 = path.stat().st_mtime_ns
            t0 = time.time()
            app.raw_click(node)
            t_file = t_ui = None
            deadline = t0 + 60
            while time.time() < deadline and (t_file is None or t_ui is None):
                if t_file is None and path.stat().st_mtime_ns != m0:
                    t_file = time.time() - t0
                if t_ui is None and app.find_all(role="list item", name=f"订阅 perf/# Q0 {want}"):
                    t_ui = time.time() - t0
                time.sleep(0.005)
            toggles.append(dict(file_s=round(t_file, 3) if t_file else None, ui_s=round(t_ui, 3) if t_ui else None))
            time.sleep(0.5)
        res["toggle_subscription"] = toggles
        res["toggle_cpu"] = env.sampler.summary("toggle")
        d = P.load_yaml(path)
        res["after_save"] = dict(file_mb=round(path.stat().st_size / 1048576, 2), legacy_messages=len(d.get("messages") or []),
                                 templates=len(d.get("templates") or []))
        # quick command (use_template -> save) latency
        env.phase("quick-cmd")
        qc = []
        for _ in range(3):
            btn = app.find(role="push button", pattern=r"^tpl-\d{4}$")
            m0 = path.stat().st_mtime_ns
            t0 = time.time()
            app.raw_click(btn)
            wait_until(lambda: path.stat().st_mtime_ns != m0, 60, interval=0.005, desc="save after use")
            t_file = time.time() - t0
            app.find(role="status bar", contains="已加载", timeout=30)
            qc.append(dict(file_s=round(t_file, 3), toast_s=round(time.time() - t0, 3)))
            time.sleep(2.5)
        res["quick_command"] = qc
        # template drawer and scheduled dialog with 2000 templates
        app.escape_all(2)
        env.phase("drawer")
        t0 = time.time()
        app.raw_click(app.find(role="push button", name="管理模板"))
        app.find(name="搜索模板", timeout=120)
        res["drawer_open_s"] = round(time.time() - t0, 2)
        time.sleep(1)
        res["drawer_items_in_tree"] = len(app.find_all(role="list item", pattern=r"^模板 "))
        t0 = time.time()
        app.fill(Query(name="搜索模板"), "tpl-19")
        wait_until(lambda: len(app.find_all(role="list item", pattern=r"^模板 ")) <= 111, 120, desc="drawer search")
        res["drawer_search_s"] = round(time.time() - t0, 2)
        res["drawer_cpu"] = env.sampler.summary("drawer")
        # close the drawer by clicking its overlay
        ox, oy = app.window_origin()
        app.xdo("mousemove", str(ox + 300), str(oy + 450), "click", "1")
        time.sleep(1)
        app.escape_all(2)
        env.phase("schedule-dialog")
        t0 = time.time()
        app.raw_click(app.find(role="push button", name="定时发布"))
        app.find(role="push button", name="开始发布", timeout=120)
        res["schedule_dialog_open_s"] = round(time.time() - t0, 2)
        time.sleep(1)
        t0 = time.time()
        node = app.find(role="list item", name="命令 tpl-0001")
        app.raw_click(node)
        app.find(role="list item", name="命令 tpl-0001 已选 #1", timeout=120)
        res["schedule_select_click_s"] = round(time.time() - t0, 2)
        t0 = time.time()
        app.raw_click(app.find(role="check box", name="全选"))
        app.find(role="list item", name=f"命令 tpl-{n_templates:04d} 已选 #{n_templates}", timeout=120)
        res["schedule_select_all_s"] = round(time.time() - t0, 2)
        res["schedule_dialog_cpu"] = env.sampler.summary("schedule-dialog")
        app.escape_all(3)
        time.sleep(1)
        # inbound load with the big data set (compare with exp1 r1000)
        app.connect_and_wait()
        res["r1000"] = run_step(env, "r1000", rate=1000, duration=20)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp5", res)


# ---------------------------------------------------------------------------
# 6. scheduled publishing at the minimum interval


def exp6(n_templates=5, seconds=60):
    tpls = [template(i, 1, f"sched-{i}", f"perf/sched/{i}", json.dumps({"n": i, "on": True})) for i in
            range(1, n_templates + 1)]
    env = Env("exp6", servers=[server(1, "本地")], subscriptions=[sub(1, 1, "perf/in/#")], templates=tpls)
    app = env.app
    res = dict(templates=n_templates)
    try:
        app.connect_and_wait()
        res["idle"] = idle_probe(env, seconds=3, n=2)
        app.click(Query(role="push button", name="定时发布"))
        app.click(Query(role="check box", name="全选"))
        app.fill(Query(name="发送间隔"), "10")
        app.click(Query(role="radio button", name="无限循环"))
        spy = LoadGen("sub", filter="perf/sched/#", duration=seconds + 15, log_name="sub-exp6.jsonl")
        time.sleep(1)
        env.phase("dialog-open")
        t_start = time.time()
        app.click(Query(role="push button", name="开始发布"), settle=0)
        app.find(role="status bar", pattern=r"^定时发布 运行中 ")
        time.sleep(15)
        res["dialog_open_probe"] = env.sampler.summary("dialog-open")
        app.click(Query(role="push button", name="后台运行"))
        app.wait_gone(role="dialog")
        env.phase("background")
        probes = []
        while time.time() - t_start < seconds:
            time.sleep(5)
            probes.append(env.app.filter_probe(timeout=60))
        res["probes"] = probes
        res["background"] = env.sampler.summary("background")
        progress = app.find(role="status bar", pattern=r"^定时发布进度 ").name
        res["progress_label"] = progress
        env.phase("stop")
        t0 = time.time()
        app.click(Query(role="push button", name="停止"), settle=0)
        app.find(role="status bar", contains="已停止定时发布", timeout=30)
        res["stop_s"] = round(time.time() - t0, 2)
        t_stop = time.time()
        fin = spy.wait(seconds + 30)
        res["broker"] = fin
        res["run_s"] = round(t_stop - t_start, 1)
        res["rate_at_broker"] = round(fin["count"] / (t_stop - t_start), 1)
        # last message seen by the broker relative to the stop click (t0)
        res["last_msg_after_stop_click_ms"] = fin.get("last_ms", 0) - int(t0 * 1000)
        res["count_label"] = app.message_count()
        res["rss_end"] = env.rss()
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp6", res)


# ---------------------------------------------------------------------------
# 7. soak


def soak(env: Env, phase: str, rate: int, seconds: int, seq_start=0) -> dict:
    env.phase(phase)
    lg = LoadGen(topic="perf/a", rate=rate, duration=seconds, seq_start=seq_start, log_name=f"loadgen-{env.tag}-{phase}.jsonl")
    t0 = time.time()
    probes = []
    while lg.proc.poll() is None:
        time.sleep(30)
        if lg.proc.poll() is None:
            p = env.app.filter_probe(timeout=60)
            p["at_s"] = round(time.time() - t0)
            probes.append(p)
    fin = lg.wait()
    rows = env.sampler.window(phase)
    # linear fit of RSS over the second half of the run
    half = rows[len(rows) // 2:]
    xs = [r["t"] for r in half]
    ys = [r["rss_mb"] for r in half]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / max(sum((x - mx) ** 2 for x in xs), 1e-9)
    t_seen = wait_seq(env.app, fin["seq_last"], 120)
    return dict(phase=phase, rate=rate, seconds=seconds, sent=fin["sent"], probes=probes,
                drained_s=round(t_seen - time.time(), 1) if t_seen else None,
                rss_first=rows[0]["rss_mb"], rss_at_60s=next((r["rss_mb"] for r in rows if r["t"] - rows[0]["t"] >= 60), None),
                rss_last=rows[-1]["rss_mb"], rss_max=max(r["rss_mb"] for r in rows),
                rss_slope_mb_per_min_2nd_half=round(slope * 60, 3),
                threads=(rows[0]["threads"], rows[-1]["threads"]), fds=(rows[0]["fds"], rows[-1]["fds"]),
                rss_series=[(round(r["t"] - rows[0]["t"]), r["rss_mb"]) for r in rows[::10]],
                **lag_stats(env, phase, seq_start), load=env.sampler.summary(phase))


def exp7():
    seconds = int(os.environ.get("PERF_SOAK_S", "300"))
    script_rate = int(os.environ.get("PERF_SCRIPT_RATE", "100"))
    script_seconds = int(os.environ.get("PERF_SCRIPT_SOAK_S", "180"))
    res = {}
    env = Env("exp7", sample_interval=2.0, lag_interval=2.0, **single_server())
    try:
        env.app.connect_and_wait()
        res["plain"] = soak(env, "soak", 500, seconds)
        log("soak", json.dumps({k: v for k, v in res["plain"].items() if k not in ("load", "rss_series")}))
        res["plain"]["after"] = watch_rss(env, "after", 30)
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res["plain_close"] = env.close()
    env = Env("exp7-script", sample_interval=2.0, lag_interval=2.0,
              **single_server(scripts=[script(1, 1, "接收标记", TAG_SCRIPT)]))
    try:
        env.app.connect_and_wait()
        res["script"] = soak(env, "soak-script", script_rate, script_seconds)
        log("soak-script", json.dumps({k: v for k, v in res["script"].items() if k not in ("load", "rss_series")}))
        res["script"]["after"] = watch_rss(env, "after", 20)
    except Exception:
        res["error_script"] = traceback.format_exc()
        log(res["error_script"])
    res["script_close"] = env.close()
    save("exp7", res)


# ---------------------------------------------------------------------------
# 8. resilience


def status_name(app: PerfApp) -> str:
    try:
        return app.find(role="status bar", pattern=r"^连接状态 ", timeout=10).name
    except Exception as err:
        return f"? {err}"


def exp8(cycles=40):
    env = Env("exp8", **single_server())
    app = env.app
    res = {}
    try:
        app.connect_and_wait()
        # a) broker killed under load
        env.phase("load")
        lg = LoadGen(topic="perf/a", rate=1000, duration=60, log_name="loadgen-exp8.jsonl")
        time.sleep(8)
        seq_before = parse_seq(app.top_message())
        env.phase("broker-down")
        t_kill = time.time()
        broker.kill()
        try:
            app.find(role="status bar", pattern=r"^连接状态 (连接错误|未连接)", timeout=60)
            res["ui_status_after_kill_s"] = round(time.time() - t_kill, 2)
        except Exception as err:
            res["ui_status_after_kill_s"] = f"no change: {str(err).splitlines()[0]}"
        res["ui_status_broker_down"] = status_name(app)
        res["toasts_broker_down"] = [n.name for n in app.find_all(role="status bar") if not n.name.startswith(("连接状态", "消息数"))]
        res["probe_broker_down"] = app.filter_probe()
        time.sleep(5)
        env.phase("broker-restarted")
        broker.start()
        t_restart = time.time()
        time.sleep(15)
        res["ui_status_15s_after_restart"] = status_name(app)
        seq_mid = parse_seq(app.top_message())
        time.sleep(2)
        res["auto_resumed"] = parse_seq(app.top_message()) != seq_mid
        res["alive_after_broker_restart"] = app.alive()
        app.screenshot("exp8-broker-restarted")
        # manual reconnect
        t0 = time.time()
        btn = app.find(role="push button", pattern=r"^(连接|重新连接|断开)$")
        res["button_after_restart"] = btn.name
        if btn.name == "断开":
            app.raw_click(btn)
            app.find(role="push button", name="连接", timeout=20)
            btn = app.find(role="push button", name="连接")
        app.raw_click(btn)
        app.find(role="status bar", pattern=r"^连接状态 已连接", timeout=30)
        res["manual_reconnect_s"] = round(time.time() - t0, 2)
        time.sleep(3)
        res["flow_after_reconnect"] = dict(before=seq_before, after=parse_seq(app.top_message()))
        lg.stop()
        lg.wait(10)
        res["loadgen_after_restart"] = lg.final() or lg.lines[-1:]
        # b) connect / disconnect cycles under light load
        env.phase("cycles")
        lg = LoadGen(topic="perf/a", rate=200, duration=600, log_name="loadgen-exp8-cycles.jsonl")
        before = dict(rss=env.rss(), threads=P._status(app.proc.pid).get("Threads"),
                      fds=len(os.listdir(f"/proc/{app.proc.pid}/fd")), broker_sockets=P.broker_sockets(app.proc.pid))
        times = []
        rss_series = [(0, env.rss())]
        for i in range(cycles):
            if i and i % 10 == 0:
                rss_series.append((i, env.rss()))
            t0 = time.time()
            app.raw_click(app.find(role="push button", name="断开"))
            app.find(role="status bar", pattern=r"^连接状态 未连接", timeout=30)
            t1 = time.time()
            app.raw_click(app.find(role="push button", name="连接"))
            app.find(role="status bar", pattern=r"^连接状态 已连接", timeout=30)
            times.append((round(t1 - t0, 3), round(time.time() - t1, 3)))
        time.sleep(3)
        after = dict(rss=env.rss(), threads=P._status(app.proc.pid).get("Threads"),
                     fds=len(os.listdir(f"/proc/{app.proc.pid}/fd")), broker_sockets=P.broker_sockets(app.proc.pid))
        rss_series.append((cycles, after["rss"]))
        res["cycles"] = dict(n=cycles, before=before, after=after, rss_series=rss_series,
                             disconnect_s_median=statistics.median(t[0] for t in times),
                             connect_s_median=statistics.median(t[1] for t in times),
                             connect_s_max=max(t[1] for t in times))
        # c) rapid toggling without waiting for the state change
        env.phase("rapid")
        for _ in range(15):
            try:
                b = app.find(role="push button", pattern=r"^(连接|断开)$", timeout=5)
                app.raw_click(b)
            except Exception:
                pass
            time.sleep(0.05)
        time.sleep(5)
        res["rapid"] = dict(ui_status=status_name(app), button=app.find(role="push button", pattern=r"^(连接|断开)$").name,
                            broker_sockets=P.broker_sockets(app.proc.pid), alive=app.alive())
        lg.stop()
        res["final_rss"] = env.rss()
    except Exception:
        res["error"] = traceback.format_exc()
        log(res["error"])
    res.update(env.close())
    save("exp8", res)


def main():
    broker.takeover()
    try:
        for e in EXPS:
            log("=== experiment", e)
            globals()[f"exp{e}"]()
    finally:
        broker.kill()
        os.system("pkill -f mqtt-load >/dev/null 2>&1")


if __name__ == "__main__":
    main()
