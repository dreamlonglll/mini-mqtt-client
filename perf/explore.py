#!/usr/bin/env python3
"""Calibration: thread names, cost of the AT-SPI probes, idle click latency."""
import time
from perflib import *  # noqa: F403
from perflib import _threads

broker = QuietBroker()
broker.takeover()
data_dir = WORK / "data-explore"
write_data(data_dir, servers=[server(1, "本地")], subscriptions=[sub(1, 1, "perf/#")])
app = PerfApp(data_dir, "explore")
print("startup", app.timed_start())
app.connect_and_wait()
pid = app.proc.pid
print("threads", sorted({(c, thread_group(c, t, pid)) for t, (c, _) in _threads(pid).items()}))
t = time.time(); n = len(app.nodes()); print("full tree walk", n, "nodes", round(time.time() - t, 3), "s")
for _ in range(3):
    print("filter probe idle", app.filter_probe())
lg = LoadGen(topic="perf/a", rate=1000, count=3000, size=64)
lg.wait()
time.sleep(1)
t = time.time(); print("top", app.top_message(), round(time.time() - t, 3))
t = time.time(); print("top (cached)", app.top_message(), round(time.time() - t, 3))
t = time.time(); print("count", app.message_count(), round(time.time() - t, 3))
t = time.time(); n = len(app.nodes()); print("full tree walk", n, "nodes", round(time.time() - t, 3), "s")
for _ in range(3):
    print("filter probe 1000 msgs", app.filter_probe())
print(app.dump()[:3000])
app.stop()
broker.kill()
