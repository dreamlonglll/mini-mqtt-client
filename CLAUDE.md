# CLAUDE.md

这是一个基于 **Rust + GPUI Kit** 开发的跨平台 MQTT 调试客户端（原生 GPU 渲染桌面应用，无 WebView）。

> v2.0 由 Tauri 2 + Vue 3 版本完整重写而来。`docs/阶段开发文档/`、`docs/阶段进度文档/`、`docs/development-plan.md` 记录的是旧版（Vue）的开发历史，仅作功能需求参考。

## 项目概述

Mini MQTT Client 用于 MQTT 消息的调试和测试。支持多服务器管理、消息发布/订阅、命令模板、定时发布、JavaScript 预处理脚本、按服务器的环境变量、中英文界面和检查更新等功能。

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| UI 框架 | GPUI + GPUI Kit (`gpui-kit` 0.8) | `use gpui_kit::*` 即 GPUI，组件在 `gpui_kit::component` |
| MQTT | rumqttc 0.25 | tokio 运行时；v3.1.1 / v5；TCP / TLS(rustls + ring) / WebSocket |
| 脚本引擎 | boa_engine 0.21 | 纯 Rust JavaScript 引擎 |
| 加密 | RustCrypto | aes-gcm、cbc、sha1、sha2、md-5、hmac、crc32fast |
| 数据存储 | YAML (serde_yaml) | 与旧版 Tauri 的 `data.yaml` 完全兼容 |
| 界面文案 | rust-i18n 4 + sys-locale | `locales/app.yml`，与 GPUI Kit 组件共用同一个全局 locale |

## 项目结构

```
src/
├── main.rs                # 入口：tr! 宏、rust_i18n::i18n!、初始化 gpui_kit、打开主窗口、panic 日志
├── assets.rs              # AssetSource：默认组件图标 + icon_assets! 选取的 Lucide 图标
├── i18n.rs                # Language（auto / zh-CN / en-US）、系统语言检测、MINI_MQTT_LANG、键集测试
├── state.rs               # AppState（Entity）：服务器、连接状态、消息流、业务操作、事件
├── backend/               # 与 UI 无关，可单元测试
│   ├── models.rs          # 数据模型（serde 字段与旧版一致，flatten 的 extra 透传未知字段）
│   ├── storage.rs         # Storage：YAML 读写（原子写入 + fsync）、各实体 CRUD（含环境变量）
│   ├── config.rs          # 数据目录（com.minimqtt.client）、config.yaml（data_path、theme、language、message_limit）
│   ├── env.rs             # {{变量名}} 单遍替换、变量名校验（全项目唯一实现）
│   ├── update.rs          # 检查更新：rustls 直连 GitHub Releases API、版本比较
│   ├── color.rs           # 订阅颜色解析（#RGB / #RRGGBB / #RRGGBBAA / rgb() / rgba()）
│   ├── logger.rs          # LogManager：logs/error-YYYY-MM-DD.log
│   ├── mqtt.rs            # MqttManager：tokio 运行时 + 事件通道 MqttEvent，断线退避重连
│   ├── payload.rs         # 格式识别、HEX 编解码、JSON 格式化、换行标记
│   ├── topic.rs           # 发布/订阅 Topic 校验、订阅通配符匹配
│   ├── errors.rs          # MQTT 错误 → 友好提示（mqtt_errors.* 文案键；英文片段 + OS 错误码，Windows 的系统错误是本地化文字）
│   └── script/            # run_script / run_chain / validate + crypto.* / zlib.* / env.* 原生实现
└── ui/
    ├── app_view.rs        # 根视图：侧边栏 | 头部 + 消息列表 + 发布面板 + 快捷命令栏
    ├── sidebar.rs         # 版本号 / 更新标记、可折叠的服务器与订阅列表、主题切换
    ├── header.rs          # 服务器信息栏（状态、连接/断开）
    ├── message_list.rs    # 虚拟列表（ListState），搜索/过滤
    ├── message_detail.rs  # 消息详情对话框（文本/JSON/HEX）
    ├── publish_panel.rs   # 发布面板
    ├── command_bar.rs     # 快捷命令栏
    ├── scheduled.rs       # Scheduler（定时发布任务）+ 对话框
    ├── server_dialog.rs / subscription_dialog.rs（新增 / 编辑 + 颜色）/ template_dialog.rs
    ├── template_drawer.rs # 模板管理（右侧 Sheet）
    ├── env_drawer.rs      # 环境变量管理（右侧 Sheet）+ 新增 / 编辑对话框
    ├── script_dialog.rs   # 脚本管理（编辑器、测试运行、文件导入、函数帮助）
    ├── settings_dialog.rs # 系统设置（主题、语言、消息上限、检查更新、数据目录、日志）
    └── common.rs          # 共用小组件（card、chip、segmented、confirm / ask、notify…）
locales/app.yml            # 界面文案：每个键同时给出 zh-CN 与 en（rust-i18n 格式 v2）
```

## 常用命令

```bash
cargo run                       # 开发运行
cargo build --release           # 发布构建
cargo test                      # 单元测试
cargo test -- --ignored         # 集成测试（需本地 Broker：1883 TCP、8083 WebSocket；MQTT_TEST_CA 可测 8883 TLS）
cargo clippy && cargo fmt       # 提交前检查
./e2e/run.sh                    # 端到端测试（Xvfb + mosquitto + AT-SPI，见 e2e/README.md）
python e2e/run_win.py           # Windows 上的同一套端到端测试（UI Automation + 投递窗口消息）
```

调试时可用 `MINI_MQTT_DATA_DIR=<目录>` 指向独立的数据目录，`MINI_MQTT_LANG=zh-CN|en-US|auto` 指定启动语言，`MINI_MQTT_NO_UPDATE_CHECK=1` 关闭启动后的自动检查更新，`MINI_MQTT_REDUCE_MOTION=1` 关闭对话框 / 抽屉等过渡动画（E2E 默认设置），`RUST_LOG=info` 查看日志。`MINI_MQTT_TEST_GITHUB=1 cargo test -- --ignored latest_release_from_github` 会真实请求一次 GitHub（不设该变量时这个用例什么也不做，CI 的 `--include-ignored` 不受 GitHub 限流影响）。

## 架构要点

1. **状态**：`AppState` 是唯一的业务状态 Entity；视图通过 `cx.observe(&state, ..)` 刷新，通过 `AppEvent` 订阅特定事件（`Toast`、`MessagesChanged(server_id)`、`FillPublish`、`ServerDeleted`、`LanguageChanged`）。新增消息只发 `MessagesChanged`，不调用 `notify`，避免整窗重绘。
2. **MQTT**：`MqttManager` 在自有 tokio 运行时中跑事件循环，UI 侧用 `try_publish/try_subscribe` 非阻塞调用。每个连接带 generation，旧连接的事件会被丢弃；句柄先登记再启动事件循环。连上后意外断开进入 `Reconnecting` 并退避重连（0.5 s 起翻倍，最长 10 s）；首次连接失败、CONNACK 拒绝、包超过 64 MB 直接报错停止。每次 `Connected` 后由 `AppState` 自动订阅已启用的订阅。
3. **脚本**：脚本在 `background_executor` 上执行（Boa Context 每次新建）；接收消息按到达顺序串行处理。脚本约定：定义 `process(payload, topic)`，返回值替换 payload（对象自动 JSON 序列化，`Uint8Array` 按原始字节，undefined 保持原样）。脚本链在 `script::Payload`（`Text` / `Bytes`）上运行：`payload` 是文本视图（字节按 UTF-8 宽松解码），`payloadBytes` 是字节视图；HEX 格式发布时用 `TextForm::Hex`，文本视图是 HEX。返回的字符串与输入的 `payload` 相同时保留输入原样，只读脚本不会破坏二进制消息。脚本返回字节时，发布直接发送这些字节，不再按发布格式编码。`env` 对象的数据经线程局部变量交给原生函数。
4. **环境变量**：`{{变量名}}` 只由 `backend/env.rs::replace` 替换：发布（手动、模板、定时都走 `AppState::publish`）先替换 Topic / Payload 再校验、执行脚本；订阅在入库前替换；模板存原文、发送时替换。
5. **对话框**：通过 `window.open_dialog` / `open_sheet_at` 打开；对话框内容是预先 `cx.new` 的视图实体（builder 可能被多次调用，不要在 builder 里创建实体）。确认按钮用 `DialogAction` 触发 `on_ok`（返回 `false` 可阻止关闭）。
6. **无障碍 / E2E**：E2E 测试通过 AT-SPI 按无障碍名称（中文文案）定位控件，`e2e/run.sh` 以 `MINI_MQTT_LANG=zh-CN` 固定中文界面。纯图标按钮必须设置 `accessibility_label`，列表行/状态用 `.role(..).aria_label(..)` 暴露文本（`Role::Label` 的名称取自 value，需要文字时用 `Role::Status`）。不要使用 GPUI Kit 的 `NumberInput`（会重复声明焦点，debug 下触发断言），改用 `common::number_field`。E2E 设 `MINI_MQTT_REDUCE_MOTION=1`：对话框 / 抽屉滑入时无障碍树给的是上一帧的坐标，点击会落到别处（CI 上 T08 等偶发失败的原因）；driver 点击前还会等控件位置稳定，确认框（`alert`）里的按钮用 `find_in` 在框内找。Windows 上 `e2e/run_win.py` 用 `driver_win.py` 跑同一套场景：UI Automation 读树（AccessKit 导出的 ARIA 角色换算成 AT-SPI 角色名），点击 / 按键是投递给窗口的消息，不动真实鼠标和焦点，窗口放在 `E2E_MONITOR` 指定的屏幕（默认最大的那块）。
7. **主题**：`Theme::change` 之后用 `Theme::update` 覆盖品牌蓝色（`state.rs` 的 `apply_brand_colors`）；「跟随系统」时 `AppView` 监听窗口外观变化实时同步。
8. **国际化**：界面上的所有文字用 `tr!("键", 参数 = 值)`（返回 `SharedString`，参数占位符为 `%{参数}`）从 `locales/app.yml` 取，新增键必须同时写 `zh-CN` 与 `en`；改中文文案前先查 `e2e/test_app.py` 是否按它定位控件。`i18n.rs` 的测试会检查两种语言的键集与占位符一致、源码里引用的字面量键都存在。语言切换后发 `LanguageChanged`，常驻视图要刷新存在状态里的文字（如输入框占位符）。按动态拼出的键取文（`tr!(format!(..))`）时，要在对应模块补测试确认键存在。
9. **数据兼容**：每个实体和 `AppData` / `AppConfig` 都有 `#[serde(flatten)] extra`，未建模的字段原样写回；编辑时以已存的对象为底稿（`..base`），不要整体重建。旧版的主题 / 语言 / 消息上限存在 WebView localStorage，读不到。
10. **检查更新**：`backend/update.rs` 用 rustls（ring）手写一次 HTTPS GET（不要引入 reqwest / ureq 等 HTTP crate，避免 aws-lc / cmake / NASM），在独立线程执行；自动检查（启动 5 秒后）失败只记 `log::warn`。
11. **长列表**：条数不受控的列表一律虚拟化，只渲染可见行（一次性渲染 2000 张模板卡片曾让打开抽屉卡 2 秒多、内存涨 460 MB）。行高可变用 `list` + `ListState`（消息列表、模板抽屉；数据变化时 `reset`，过滤条件不变则恢复滚动位置），行高一致用 `uniform_list` + `UniformListScrollHandle`（定时发布的命令列表与日志：每行文字单行截断，行要 `.w_full()`，因为行是单独布局、不会像 flex 子项那样自动拉伸）。列表数据缓存在视图里、随状态变化重建，不要在 `render` 里每帧重新读取和过滤。屏幕外的行不在无障碍树里，E2E / perf 按可见行断言。
12. **视图缓存**：收消息时整窗每秒重绘多次，侧边栏用 `cached(..)` 嵌入（`app_view.rs`），只在被 notify 时重建，所以它显示的内容必须来自会 notify 它的状态。GPUI 每帧从 prepaint 重建无障碍树，复用的缓存子树会从树里消失，因此 `window.is_a11y_active()` 时不缓存（读屏接入 / 断开时 GPUI 会整窗刷新）。E2E 跑在 AT-SPI 下，走的是不缓存的路径。

## 脚本引擎 API

脚本中可通过 `crypto` 对象访问加密工具（参数既可是字符串也可是 Uint8Array/数组）：

- 编码：`stringToBytes` / `bytesToString` / `bytesToBase64` / `base64ToBytes` / `bytesToHex` / `hexToBytes`
- 哈希：`sha256` / `sha1` / `md5` / `hmacSha256(key, data)` / `crc32`（返回小写 hex）
- AES（Base64）：`aesGcmEncrypt(text, keyB64, ivB64?)` / `aesGcmDecrypt` / `aesCbcEncrypt` / `aesCbcDecrypt`
- AES（Hex）：`aesGcmEncryptHex` / `aesGcmDecryptHex` / `aesCbcEncryptHex` / `aesCbcDecryptHex`
- 其他：`randomBytes(n)` / `generateKey(bits)` / `generateIv(len)` / `xor(data, key)`；全局 `atob` / `btoa` / `console.*`
- 环境变量：`env.变量名`、`env.get(name)`（不存在为 undefined）、`env.replace(text)`（替换 `{{变量名}}`）、`env.all()`（`[{ name, value }]`）；同名时方法优先

密文格式为 `iv || ciphertext(+tag)`；GCM IV 12 字节，CBC IV 16 字节（PKCS#7）。

压缩用 `zlib` 对象（命名同 Node.js，参数可为字符串或字节，返回 Uint8Array）：`gzip(data, level?)` / `gunzip`、`deflate` / `inflate`（zlib 格式）、`deflateRaw` / `inflateRaw`、`unzip`（按头部识别 gzip / zlib）。level 取 0~9，默认 6；解压结果上限 64 MB，截断的数据会报错。二进制消息从全局 `payloadBytes` 读取，例如 `crypto.bytesToString(zlib.gunzip(payloadBytes))`。

## 开发注意事项

1. 修改 `backend/models.rs` 时保持 serde 兼容（新字段加 `#[serde(default)]`），用户的旧 `data.yaml` 必须能加载。
2. GPUI Kit API 以 `~/.cargo/registry/src/*/gpui-component-0.8.*` 和 `gpui-base-0.8.*` 源码为准；常用 trait：`ActiveTheme`、`Sizable`、`StyledExt`、`Disableable`、`ButtonVariants`、`WindowExt`。
3. Linux 构建依赖：`libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev`。
4. Windows release 构建隐藏控制台窗口（`windows_subsystem = "windows"`），图标由 `build.rs` 通过 winresource 嵌入。
5. UI 模块（`state.rs`、`ui/*`）的测试模块不要写 `use super::*`：会把 GPUI 的 `test` 属性宏带进来，`#[test]` 展开时报 recursion limit。按需显式导入。
6. 仓库里有一个误建的、名为 `main` 的 tag（指向 2af22f5，旧版早期的提交）。`git log main..x`、`git diff main` 这类写法会按 tag 解析、结果是错的；比较与推送时写全名：`refs/heads/main`、`git push origin refs/heads/main`。

## CI / 发布

| Workflow | 触发 | 作用 |
|----------|------|------|
| `.github/workflows/ci.yml` | push 到 main、PR（只改文档不跑） | Linux：fmt、clippy、单元 + 集成测试（mosquitto）、e2e、Linux 打包冒烟；Windows：clippy、测试、release 构建；macOS：clippy、测试、universal `.app` |
| `.github/workflows/release.yml` | 推 `v*` tag；手动触发 = 给已有 release 补建某平台 | 建 draft release → 三平台构建上传 → 算 `SHA256SUMS.txt` → 转正 |
| `.github/workflows/cache-warm.yml` | main 上 `Cargo.toml` / `Cargo.lock` 变化、每周一次 | 在 main 作用域预热 release 用的 Rust 缓存（tag 运行只读） |

发布产物（打包脚本在 `scripts/`）：

| 平台 | 产物 | 脚本 |
|------|------|------|
| Windows | `mini-mqtt-client_<版本>_x64-setup.exe`、`_x64-portable.zip` | `scripts/windows-installer.nsi`（NSIS，**UTF-8 带 BOM**；安装身份沿用 1.x Tauri 版，原地升级） |
| macOS | `mini-mqtt-client_<版本>_universal.dmg` | `scripts/macos/bundle.sh`（ad-hoc 签名，bundle id `com.minimqtt.client`）、`scripts/macos/make-dmg.sh` |
| Linux | `mini-mqtt-client_<版本>_x86_64.AppImage`、`_linux-x86_64.tar.gz` | `scripts/linux/package.sh`（AppDir 根上全用相对软链，`.DirIcon` 断链会被 AppImage 目录收录测试判 FATAL） |

Windows / Linux 构建失败时 release 不公开；macOS 失败不挡发布，事后用手动触发（`platforms=macos`）补建 dmg。

发版步骤：

1. 改 `Cargo.toml` 的 `version`（唯一的版本号来源），运行 `cargo check` 同步 `Cargo.lock`，与发布说明一起单独提交 `chore: 发布 vX.Y.Z`。
2. 发布说明写在 `.github/release-notes/vX.Y.Z.md`（随 tag 的代码一起，release.yml 直接取用；没有就按提交标题生成一份占位，需事后 `gh release edit vX.Y.Z --notes-file <文件>` 补写）。
3. 打 tag 并推送：`git tag vX.Y.Z && git push origin refs/heads/main refs/tags/vX.Y.Z`。tag 必须等于 `v<Cargo.toml 版本>`，否则 create-release 直接失败；tag 含 `-`（如 `v2.1.0-beta.1`）自动标为 pre-release。
4. `gh run watch <run-id> --exit-status` 等待；失败的平台修好原因后在该次运行上 Re-run failed jobs。
