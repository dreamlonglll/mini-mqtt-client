# 端到端（E2E）测试

在真实的 X11 显示环境中启动**真实的应用程序二进制**，像用户一样操作界面，并从三个独立的角度验证结果：

| 观察面 | 方式 |
|--------|------|
| 界面 | 通过 AT-SPI 读取无障碍树（AccessKit 导出）：控件名称、选中/启用状态、消息行、连接状态、提示消息 |
| Broker | 私有 mosquitto 上的 `mosquitto_sub` 旁路订阅，核对应用实际发出的 Topic / QoS / Retain / 字节内容 |
| 持久化 | 直接解析应用写出的 `data.yaml`、`config.yaml` 与 `logs/*.log` |

输入全部是真实的 X 事件（`xdotool` 鼠标点击、键盘、剪贴板粘贴），读取输入框内容同样通过「全选 + 复制」完成。

## 运行

```bash
./e2e/run.sh
```

脚本会自动：生成临时 CA 与服务器证书 → 启动私有 mosquitto（TCP `18830`、WebSocket `18083`、TLS `18883`）→ 启动 Xvfb → 在独立的 D-Bus 会话中启用 AT-SPI → 运行 `e2e/test_app.py`。

结果输出到 `target/e2e/`：

- `report.md` / `report.json`：每个场景的结果与耗时
- `shots/Txx-pass.png` / `Txx-fail.png`：每个场景结束时的截图
- `Txx-failure.txt`：失败时的调用栈与当时的完整无障碍树
- `app.log`、`broker.log`：应用与 broker 日志

常用环境变量：

| 变量 | 说明 |
|------|------|
| `E2E_BIN` | 被测二进制，默认 `target/debug/mini-mqtt-client`（不存在则自动构建） |
| `E2E_WORK_DIR` | 输出目录，默认 `target/e2e` |
| `E2E_ONLY` | 只运行指定场景，如 `T01,T05` |
| `E2E_DISPLAY` | 使用的 X display，默认 `:97` |

`run.sh` 固定以 `MINI_MQTT_LANG=zh-CN` 启动应用：场景按中文无障碍名称定位控件，不能随机器语言（CI 上常为 `C` / `en_US`）切到英文界面；同时设置 `MINI_MQTT_NO_UPDATE_CHECK=1`，测试不访问网络检查更新。

依赖（Ubuntu/Debian）：

```bash
sudo apt-get install -y xvfb mosquitto mosquitto-clients xdotool xclip x11-apps imagemagick openssl \
  dbus at-spi2-core python3-pyatspi python3-yaml mesa-vulkan-drivers fonts-noto-cjk
```

## 场景

| ID | 场景 | 主要验证点 |
|----|------|-----------|
| T01 | 启动并加载旧版数据 | 旧 Tauri 格式 `data.yaml`（含 v1.5+ 的环境变量、订阅颜色等字段）中的服务器、订阅（启停状态）、模板正确展示 |
| T02 | 连接与自动订阅 | 连接状态、自动订阅已启用 Topic、接收后脚本生效、停用的订阅收不到消息 |
| T03 | 接收格式 | 中文文本、二进制字节原样保留（经过脚本也不损坏）、重连后 Retain 标记 |
| T04 | 订阅管理 | 启停（含持久化）、新增（QoS 1）、非法通配符、重复订阅、删除 |
| T05 | 发布 | JSON/TEXT/HEX、QoS 1/2、Retain（Broker 侧核对）、JSON/HEX/通配符校验、JSON 格式化 |
| T06 | 消息列表 | 关键词搜索、方向过滤、清空 |
| T07 | 消息详情 | JSON 美化视图、HEX Dump、复制到剪贴板、复制到发布面板 |
| T08 | 预处理脚本 | 测试运行（含 sha256）、死循环保护、语法错误、发送前处理改写 payload、运行时错误不发送并写日志、启停、删除 |
| T09 | 命令模板 | 从发布面板保存、必填校验、快捷命令、抽屉搜索/分类/使用、复制、编辑、删除、使用次数 |
| T10 | 定时发布 | 勾选顺序、指定次数、发送间隔、HEX 模板按字节发送、后台运行与停止 |
| T11 | 服务器管理 | WebSocket + MQTT 3.1.1、多服务器同时连接、按服务器隔离的消息列表、编辑、复制、删除 |
| T12 | TLS 与错误处理 | mqtts + 自定义 CA、缺少 CA 时的证书错误、连接被拒绝的友好提示与错误日志 |
| T13 | 设置 | 主题切换与 `config.yaml` 持久化、数据/日志路径、日志查看与清空 |

T16、T17 排在 T13 之后、T14 之前执行（T14 重启后再核对持久化）。
| T16 | 环境变量 | 旧版写入的环境变量与订阅颜色经多次保存仍保留；抽屉中新增变量与名称校验；订阅 Topic、发布 Topic / Payload 中的 `{{变量名}}` 替换（Broker 侧核对）；脚本中的 `env.replace` / `env.get` |
| T17 | 订阅编辑与语言 | 编辑订阅的 QoS 与颜色并按新 QoS 重新订阅；设置中切换英文 / 中文立即生效并写入 `config.yaml` |
| T14 | 重启持久化 | 重启后服务器、订阅、模板、脚本、主题保持；实时消息不写入数据文件 |
| T15 | 删除服务器 | 级联删除该服务器的订阅、模板、脚本与环境变量 |

未覆盖：模板导入/导出与更改数据目录依赖系统原生文件对话框（xdg-desktop-portal），在无桌面环境的 Xvfb 中无法弹出；对应逻辑由 `cargo test` 中的单元测试覆盖。
