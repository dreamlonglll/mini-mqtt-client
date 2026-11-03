# Mini MQTT Client

English | [简体中文](README.zh-CN.md)

A lightweight, good-looking MQTT debugging client. It is a native desktop app built with **Rust + [GPUI Kit](https://gpui-kit.com)** (GPU-rendered, no WebView), available for Windows, macOS and Linux.

> Starting with v2.0 the project has been completely rewritten from Tauri 2 + Vue 3 to pure Rust. Your existing data keeps working; see [Data compatibility](#data-compatibility).

![Main window](docs/screenshots/main.png)

## Features

### MQTT connection management
- Manage multiple server configurations (add / edit / duplicate / delete)
- **MQTT 3.1.1 and 5.0**
- Four transports: `mqtt://`, `mqtts://` (TLS/SSL), `ws://` and `wss://`
- TLS with a custom CA certificate and mutual (client certificate) authentication in PEM format; the system root certificates are used when no CA is given
- Keep several servers connected at the same time, with live connection status
- Enabled topics are subscribed automatically once connected
- Keep Alive / Clean Session settings; the Client ID can be generated automatically

![Server configuration](docs/screenshots/server-config.png)

### Publishing and subscribing
- Subscribe to multiple topics with `+` / `#` wildcards (validated), each can be enabled or disabled on its own
- QoS 0/1/2 and retained messages
- Publish as JSON / TEXT / HEX, with one-click JSON formatting
- Message list: live scrolling, virtualized list (the latest 1000 messages per server), filter by direction, keyword search
- JSON / TEXT / BIN is detected automatically; message details offer text, pretty-printed JSON and HEX dump views
- Copy a message's content, or copy it into the publish panel, in one click

![Message details](docs/screenshots/messages.png)

### Command templates
- Save frequently used commands as templates, with categories and search
- The quick command bar at the bottom shows your most used templates
- Import / export templates (JSON, compatible with the previous version's format)

![Command templates](docs/screenshots/templates.png)

### Scheduled publishing
- Send several templates in a loop, in the order you ticked them or sorted by name
- Configurable interval between messages and between rounds; loop forever or for a fixed number of rounds
- Can run in the background; the publish panel shows live progress and can stop it at any time

![Scheduled publishing](docs/screenshots/scheduled-publish.png)

### Pre-processing scripts
- Built-in JavaScript engine ([Boa](https://boajs.dev/), pure Rust) with `async/await`
- Before-publish processing (e.g. encryption, framing) and after-receive processing (e.g. decryption, parsing)
- Built-in crypto helpers: AES-GCM / AES-CBC, SHA-1 / SHA-256 / MD5, HMAC-SHA256, CRC32, Base64 / Hex and more
- Code editor with syntax highlighting; **test-run** a script right away and see its `console.log` output
- Infinite-loop protection: a script that exceeds its run limit is aborted instead of freezing the UI

![Pre-processing scripts](docs/screenshots/scripts.png)

### Other
- Light / dark / follow-system theme
- Custom data location (existing data can be migrated)
- Error log recording and viewer (including crash information)
- Message timestamps with millisecond precision

## Installation

Download the package for your platform from the [Releases](../../releases) page (`<version>` is the version number, e.g. `2.0.0`):

| Platform | File | Notes |
|----------|------|-------|
| Windows | `mini-mqtt-client_<version>_x64-setup.exe` | Installer; upgrades 1.x in place |
| Windows | `mini-mqtt-client_<version>_x64-portable.zip` | Portable, unzip and run |
| macOS | `mini-mqtt-client_<version>_universal.dmg` | Apple Silicon / Intel; drag into Applications |
| Linux | `mini-mqtt-client_<version>_x86_64.AppImage` | `chmod +x` and run |
| Linux | `mini-mqtt-client_<version>_linux-x86_64.tar.gz` | Executable + `.desktop` file + icon |

`SHA256SUMS.txt` lists the checksums of all files; verify your download with `sha256sum -c SHA256SUMS.txt --ignore-missing`.

- **Windows**: the installer detects an installed older version (including the 1.x Tauri `.exe` installer) and uninstalls it first; your data is kept. If you installed 1.x from the `.msi`, uninstall it from Settings → Apps first.
- **macOS**: the app is only ad-hoc signed and not notarized. On first launch, right-click the app in Finder and choose "Open"; if macOS says it is damaged, run `xattr -dr com.apple.quarantine "/Applications/Mini MQTT Client.app"`.
- **Linux**: requires glibc 2.35 or newer (Ubuntu 22.04+ and similar) and a working Vulkan driver.

## Building from source

Requires Rust 1.92+ (installing the latest stable with [rustup](https://rustup.rs) is recommended).

```bash
git clone https://github.com/dreamlonglll/mini-mqtt-client
cd mini-mqtt-client

# Run in development mode
cargo run

# Build a release (output in target/release/)
cargo build --release
```

On Linux, install these system dependencies first (Ubuntu/Debian example):

```bash
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev
```

### Tests

```bash
# Unit tests (storage, protocol validation, crypto functions, script engine, ...)
cargo test

# Integration tests against a real broker (needs local listeners on 1883 TCP and 8083 WebSocket)
cargo test -- --ignored

# End-to-end tests: launch the real app under Xvfb, drive the UI through the accessibility tree,
# and check what the broker received and what was written to disk (see e2e/README.md)
./e2e/run.sh
```

## Data compatibility

The new version uses the same data directory and file format as the previous (Tauri) version, so your servers, subscriptions, templates and scripts load automatically after upgrading:

| Platform | Data directory |
|----------|----------------|
| Windows | `%APPDATA%\com.minimqtt.client` |
| macOS | `~/Library/Application Support/com.minimqtt.client` |
| Linux | `~/.local/share/com.minimqtt.client` |

The directory contains `data.yaml` (data), `config.yaml` (custom data path, theme) and `logs/` (error logs). You can also point the app at another directory with the `MINI_MQTT_DATA_DIR` environment variable.

Behaviour changes compared with the previous version:
- Protocol version / TLS / WebSocket settings now actually take effect (the old backend always connected with MQTT 3.1.1 over TCP)
- HEX templates are sent as bytes during scheduled publishing (the old version sent the HEX string as text)
- `aesCbcEncrypt/Decrypt` use standard single-layer PKCS#7 padding and interoperate with other systems (the old version padded twice)
- `md5` / `crc32` hash the UTF-8 bytes, so results match common tools
- When an after-receive script does not change the content, the original bytes are kept and binary messages are no longer corrupted by UTF-8 conversion
- The message list is an in-memory live view; publish records are no longer written to `data.yaml`
- The message list keeps at most 1000 messages per server with at most 128 MiB of payload in total; when messages arrive faster than they can be processed, the oldest are dropped and you are notified
- Message search matches the topic and the first 4 KB of the content; details of very large messages render only the beginning, while "Copy content" still copies everything

## Tech stack

| Layer | Technology |
|-------|------------|
| UI framework | [GPUI](https://www.gpui.rs) + [GPUI Kit](https://gpui-kit.com) (gpui-component library) |
| MQTT | [rumqttc](https://github.com/bytebeamio/rumqtt) (tokio, rustls) |
| Script engine | [Boa](https://github.com/boa-dev/boa) |
| Crypto | RustCrypto (aes-gcm, cbc, sha2, hmac, ...) |
| Storage | YAML (serde_yaml) |

## Project layout

```
mini-mqtt-client/
├── src/
│   ├── main.rs              # Entry point: initialise GPUI Kit, open the window
│   ├── assets.rs            # Icon assets (Lucide)
│   ├── state.rs             # AppState: connection status, message stream, operations
│   ├── backend/             # UI-independent logic
│   │   ├── models.rs        # Data models (compatible with the old data.yaml)
│   │   ├── storage.rs       # YAML storage
│   │   ├── config.rs        # Data directory, config.yaml
│   │   ├── logger.rs        # Error log
│   │   ├── mqtt.rs          # MQTT connection management (v3.1.1/v5, TLS, WS)
│   │   ├── payload.rs       # Format detection, HEX, JSON
│   │   ├── topic.rs         # Topic validation
│   │   ├── errors.rs        # Friendly error messages
│   │   └── script/          # JS script engine and crypto API
│   └── ui/                  # GPUI views and dialogs
├── assets/app-icon/         # Application icons
├── scripts/                 # Packaging scripts (Windows installer, macOS .app / dmg, Linux AppImage)
├── e2e/                     # End-to-end tests
├── perf/                    # Stress / performance test tools
├── docs/                    # Documentation and screenshots
└── .github/
    ├── workflows/           # CI / Release / cache warm-up
    └── release-notes/       # Release notes, one <tag>.md per version
```

## License

[MIT License](LICENSE)
