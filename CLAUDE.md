# CLAUDE.md

这是一个基于 **Rust + GPUI Kit** 开发的跨平台 MQTT 调试客户端（原生 GPU 渲染桌面应用，无 WebView）。

> v2.0 由 Tauri 2 + Vue 3 版本完整重写而来。`docs/阶段开发文档/`、`docs/阶段进度文档/`、`docs/development-plan.md` 记录的是旧版（Vue）的开发历史，仅作功能需求参考。

## 项目概述

Mini MQTT Client 用于 MQTT 消息的调试和测试。支持多服务器管理、消息发布/订阅、命令模板、定时发布和 JavaScript 预处理脚本等功能。

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| UI 框架 | GPUI + GPUI Kit (`gpui-kit` 0.8) | `use gpui_kit::*` 即 GPUI，组件在 `gpui_kit::component` |
| MQTT | rumqttc 0.25 | tokio 运行时；v3.1.1 / v5；TCP / TLS(rustls + ring) / WebSocket |
| 脚本引擎 | boa_engine 0.21 | 纯 Rust JavaScript 引擎 |
| 加密 | RustCrypto | aes-gcm、cbc、sha1、sha2、md-5、hmac、crc32fast |
| 数据存储 | YAML (serde_yaml) | 与旧版 Tauri 的 `data.yaml` 完全兼容 |

## 项目结构

```
src/
├── main.rs                # 入口：初始化 gpui_kit、打开主窗口、panic 日志
├── assets.rs              # AssetSource：默认组件图标 + icon_assets! 选取的 Lucide 图标
├── state.rs               # AppState（Entity）：服务器、连接状态、消息流、业务操作、事件
├── backend/               # 与 UI 无关，可单元测试
│   ├── models.rs          # 数据模型（serde 字段与旧版一致）
│   ├── storage.rs         # Storage：YAML 读写（原子写入）
│   ├── config.rs          # 数据目录（com.minimqtt.client）、config.yaml（data_path、theme）
│   ├── logger.rs          # LogManager：logs/error-YYYY-MM-DD.log
│   ├── mqtt.rs            # MqttManager：tokio 运行时 + 事件通道 MqttEvent
│   ├── payload.rs         # 格式识别、HEX 编解码、JSON 格式化
│   ├── topic.rs           # 发布/订阅 Topic 校验
│   ├── errors.rs          # MQTT 错误 → 中文友好提示
│   └── script/            # run_script / run_chain / validate + crypto.* 原生实现
└── ui/
    ├── app_view.rs        # 根视图：侧边栏 | 头部 + 消息列表 + 发布面板 + 快捷命令栏
    ├── sidebar.rs         # 服务器列表、订阅列表、主题切换
    ├── header.rs          # 服务器信息栏（状态、连接/断开）
    ├── message_list.rs    # 虚拟列表（ListState），搜索/过滤
    ├── message_detail.rs  # 消息详情对话框（文本/JSON/HEX）
    ├── publish_panel.rs   # 发布面板
    ├── command_bar.rs     # 快捷命令栏
    ├── scheduled.rs       # Scheduler（定时发布任务）+ 对话框
    ├── server_dialog.rs / subscription_dialog.rs / template_dialog.rs
    ├── template_drawer.rs # 模板管理（右侧 Sheet）
    ├── script_dialog.rs   # 脚本管理（编辑器、测试运行、函数帮助）
    ├── settings_dialog.rs # 系统设置
    └── common.rs          # 共用小组件（card、chip、segmented、confirm、notify…）
```

## 常用命令

```bash
cargo run                       # 开发运行
cargo build --release           # 发布构建
cargo test                      # 单元测试
cargo test -- --ignored         # 集成测试（需本地 Broker：1883 TCP、8083 WebSocket；MQTT_TEST_CA 可测 8883 TLS）
cargo clippy && cargo fmt       # 提交前检查
./e2e/run.sh                    # 端到端测试（Xvfb + mosquitto + AT-SPI，见 e2e/README.md）
```

调试时可用 `MINI_MQTT_DATA_DIR=<目录>` 指向独立的数据目录，`RUST_LOG=info` 查看日志。

## 架构要点

1. **状态**：`AppState` 是唯一的业务状态 Entity；视图通过 `cx.observe(&state, ..)` 刷新，通过 `AppEvent` 订阅特定事件（`Toast`、`MessagesChanged(server_id)`、`FillPublish`）。新增消息只发 `MessagesChanged`，不调用 `notify`，避免整窗重绘。
2. **MQTT**：`MqttManager` 在自有 tokio 运行时中跑事件循环，UI 侧用 `try_publish/try_subscribe` 非阻塞调用。每个连接带 generation，旧连接的事件会被丢弃。连接成功（`Connected`）后由 `AppState` 自动订阅已启用的订阅。
3. **脚本**：脚本在 `background_executor` 上执行（Boa Context 每次新建）；接收消息按到达顺序串行处理。脚本约定：定义 `process(payload, topic)`，返回值替换 payload（对象自动 JSON 序列化，undefined 保持原样）。
4. **对话框**：通过 `window.open_dialog` / `open_sheet_at` 打开；对话框内容是预先 `cx.new` 的视图实体（builder 可能被多次调用，不要在 builder 里创建实体）。确认按钮用 `DialogAction` 触发 `on_ok`（返回 `false` 可阻止关闭）。
5. **无障碍 / E2E**：E2E 测试通过 AT-SPI 按无障碍名称定位控件。纯图标按钮必须设置 `accessibility_label`，列表行/状态用 `.role(..).aria_label(..)` 暴露文本（`Role::Label` 的名称取自 value，需要文字时用 `Role::Status`）。不要使用 GPUI Kit 的 `NumberInput`（会重复声明焦点，debug 下触发断言），改用 `common::number_field`。
6. **主题**：`Theme::change` 之后用 `Theme::update` 覆盖品牌蓝色（`state.rs` 的 `apply_brand_colors`）。

## 脚本引擎 API

脚本中可通过 `crypto` 对象访问加密工具（参数既可是字符串也可是 Uint8Array/数组）：

- 编码：`stringToBytes` / `bytesToString` / `bytesToBase64` / `base64ToBytes` / `bytesToHex` / `hexToBytes`
- 哈希：`sha256` / `sha1` / `md5` / `hmacSha256(key, data)` / `crc32`（返回小写 hex）
- AES（Base64）：`aesGcmEncrypt(text, keyB64, ivB64?)` / `aesGcmDecrypt` / `aesCbcEncrypt` / `aesCbcDecrypt`
- AES（Hex）：`aesGcmEncryptHex` / `aesGcmDecryptHex` / `aesCbcEncryptHex` / `aesCbcDecryptHex`
- 其他：`randomBytes(n)` / `generateKey(bits)` / `generateIv(len)` / `xor(data, key)`；全局 `atob` / `btoa` / `console.*`

密文格式为 `iv || ciphertext(+tag)`；GCM IV 12 字节，CBC IV 16 字节（PKCS#7）。

## 开发注意事项

1. 修改 `backend/models.rs` 时保持 serde 兼容（新字段加 `#[serde(default)]`），用户的旧 `data.yaml` 必须能加载。
2. GPUI Kit API 以 `~/.cargo/registry/src/*/gpui-component-0.8.*` 和 `gpui-base-0.8.*` 源码为准；常用 trait：`ActiveTheme`、`Sizable`、`StyledExt`、`Disableable`、`ButtonVariants`、`WindowExt`。
3. Linux 构建依赖：`libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev`。
4. Windows release 构建隐藏控制台窗口（`windows_subsystem = "windows"`），图标由 `build.rs` 通过 winresource 嵌入。
