# 性能 / 压力测试

在 `e2e/run.sh` 准备的真实环境（私有 mosquitto、Xvfb、AT-SPI）中驱动 **release 版应用**，测量吞吐、CPU、内存与界面响应。

| 文件 | 作用 |
|------|------|
| `env.sh` | 入口：构建负载生成器（如缺失），设置端口/显示/输出目录后经 `e2e/run.sh` 运行指定 Python 脚本 |
| `perf_run.py` | 实验 `1`–`8`，以及补充实验 `2b`（脚本路径，按丢弃数统计吞吐）、`1m` / `2c`（100 万条突发 / 脚本 20k/s 的丢弃提示核对）、`3b`（清空后内存是否归还）、`3c`（HEX 详情的内存）、`4b`（多连接内存归因），用 `PERF_EXP` 选择 |
| `perflib.py` | 公共部分：安静模式 broker（替换 run.sh 的 `-v` broker）、进程采样器、负载生成器封装、界面探针 |
| `explore.py` | 校准：线程名、AT-SPI 探针开销、空闲点击延迟 |
| `compare.py` | 对比两个结果目录，输出修复前/后表格：`python3 perf/compare.py <before> <after>` |
| `loadgen/` | Rust（rumqttc）负载生成器 `mqtt-load`：`pub` 按速率发送带 `seq`/`ts` 的消息，`sub` 统计到达速率/间隔 |
| `scriptbench/` | 原样编译 `src/backend/script`，测量每条消息执行接收后脚本的开销 |

## 运行

```bash
cargo build --release                       # 被测应用
PERF_WORK=/some/dir PERF_EXP=1 perf/env.sh perf/perf_run.py
PERF_WORK=/some/dir PERF_EXP=2,3 perf/env.sh perf/perf_run.py
# 脚本引擎微基准
(cd perf/scriptbench && CARGO_TARGET_DIR=/some/dir/sb-target cargo run --release)
```

- `PERF_WORK`（默认 `target/perf`）：负载生成器构建目录、每次运行的环境目录、`results/`。
- 结果：`$PERF_WORK/results/expN.json`，以及 `samples-*.jsonl`（每秒：应用总 CPU、主线程/渲染/IO 线程 CPU、RSS、线程数、fd 数，broker/负载生成器 CPU）、`lag-*.jsonl`（界面最新一条消息的 seq 与显示延迟）、`loadgen-*.jsonl`、`app-*.log`、`shots/`。
- 其他变量：`PERF_SOAK_S`（实验 7 无脚本浸泡时长，默认 300）、`PERF_SCRIPT_RATE` / `PERF_SCRIPT_SOAK_S`（脚本浸泡速率/时长，默认 100 msg/s / 180 s）、`E2E_TCP_PORT` 等端口（默认 28830/28083/28883）、`E2E_DISPLAY`（默认 `:96`）。

## 测量方法

- **吞吐/显示延迟**：负载 payload 内含全局 `seq` 和发送时刻 `ts`（epoch ms）。`LagWatcher` 每 0.5 s 通过 AT-SPI 读取消息列表最上面一行（最新消息），得到“界面已显示到第几条”与 `now - ts`。负载结束后等到最后一条 `seq` 出现，得到积压排空时间；`app_throughput = 发送条数 / (最后一条显示时刻 - 开始时刻)`。
- **界面响应**：`filter_probe` 用 xdotool 真实点击「消息过滤 发布」再点「消息过滤 全部」，计时到 AT-SPI 显示为 pressed。空闲基线约 0.106 s（xdotool 进程 + AT-SPI 往返的固定开销）。
- **丢弃统计**：负载结束后等到最后一条出现，或最新 seq 4 s 不再变化（最后几条可能被丢弃）；丢弃数从应用日志 `logs/error-*.log` 中的“已丢弃 N 条”累加；`LagWatcher` 同时记录名称含“已丢弃”的提示。
- **注意**：探针会用 AT-SPI 遍历整棵无障碍树；当界面上有上千个节点（如 2000 个模板的抽屉/定时发布对话框）时，单次遍历本身就要数秒，此时的“交互延迟”主要是测量开销，应看主线程 CPU 秒数。
- **CPU/内存**：`/proc/<pid>/stat`、`/proc/<pid>/task/*/stat`、`/proc/<pid>/status`。线程分组：`main`（UI 线程）、`llvmpipe`（Mesa 软件渲染）、`mqtt_io`（tokio）、`Worker`（gpui 后台执行器）。

注意：Xvfb 下没有 GPU，渲染由 llvmpipe 在 CPU 上完成，渲染开销会明显高于真机；负载生成器、broker、AT-SPI 与应用共享 4 个核。
