# CLAUDE.md

这是一个基于 **Tauri 2 + Vue 3 + TypeScript + Rust** 开发的 Windows MQTT 调试客户端应用。

## 项目概述

Mini MQTT Client 是一个桌面应用程序，用于 MQTT 消息的调试和测试。支持多服务器管理、消息发布/订阅、命令模板、定时发布和预处理脚本等功能。

## 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 前端框架 | Vue 3 + TypeScript | 使用 `<script setup>` SFC |
| UI 组件库 | Element Plus | 中文友好的 Vue 3 组件库 |
| 状态管理 | Pinia | Vue 3 官方推荐 |
| 构建工具 | Vite 6 | 快速开发构建 |
| 桌面框架 | Tauri 2 | 轻量级跨平台桌面应用 |
| 后端语言 | Rust | 高性能、内存安全 |
| MQTT 库 | rumqttc | Rust MQTT 客户端库 |
| 数据存储 | YAML 文件 | 本地持久化存储 |

## 项目结构

```
mini-mqtt-client/
├── src/                           # Vue 前端源码
│   ├── components/                # Vue 组件
│   │   ├── layout/                # 布局组件 (AppLayout, Header, Sidebar, CommandBar)
│   │   ├── mqtt/                  # MQTT 功能组件 (消息列表、发布面板等)
│   │   ├── script/                # 脚本管理组件
│   │   ├── settings/              # 设置组件
│   │   └── template/              # 命令模板组件
│   ├── stores/                    # Pinia 状态管理
│   │   ├── app.ts                 # 应用状态 (主题、设置)
│   │   ├── server.ts              # 服务器配置
│   │   ├── mqtt.ts                # MQTT 连接状态
│   │   ├── message.ts             # 消息状态
│   │   ├── subscription.ts        # 订阅状态
│   │   ├── template.ts            # 模板状态
│   │   └── script.ts              # 脚本状态
│   ├── utils/
│   │   └── scriptEngine.ts        # JavaScript 脚本执行引擎 (含加密工具)
│   └── types/
│       └── mqtt.ts                # TypeScript 类型定义
├── src-tauri/                     # Tauri Rust 后端
│   └── src/
│       ├── commands/              # Tauri 命令 (server, mqtt, template, script, settings 等)
│       ├── db/                    # 数据模型和存储
│       └── mqtt/                  # MQTT 客户端封装
├── docs/                          # 开发文档
│   ├── 阶段开发文档/              # 功能需求文档
│   └── 阶段进度文档/              # 开发进度记录
└── package.json
```

## 常用命令

```bash
# 安装依赖
npm install

# 开发模式 (仅前端)
npm run dev

# 构建前端 (TypeScript 检查 + Vite 构建)
npm run build

# 开发模式 (Tauri 应用)
npm run tauri dev

# 构建 Tauri 应用
npm run tauri build
```

## 核心功能模块

1. **Server 管理** - 多 MQTT 服务器配置管理
2. **MQTT 连接** - 连接/断开、状态管理
3. **发布/订阅** - 消息发布、Topic 订阅
4. **消息展示** - 实时消息流、格式切换 (JSON/HEX/TEXT)
5. **命令模板** - 保存和快速发送常用命令
6. **定时发布** - 定时/周期发送消息
7. **预处理脚本** - JavaScript 脚本处理消息 (发送前/接收后)
8. **环境变量** - 按服务器管理环境变量，支持 `{{变量名}}` 格式替换
9. **系统设置** - 主题切换、界面配置

## 环境变量

每个 Server 可以配置独立的环境变量，使用 `{{变量名}}` 格式在以下位置进行变量替换：

- 订阅的 Topic
- 发布消息的 Topic 和 Payload
- 命令模板中的 Topic 和 Payload
- 脚本中可通过 `env.变量名` 访问

### 脚本中使用示例

```javascript
function process(payload, topic) {
  const deviceId = env.DEVICE_ID;  // 访问环境变量
  const apiKey = env.API_KEY;
  
  const data = JSON.parse(payload);
  data.deviceId = deviceId;
  
  return JSON.stringify(data);
}
```

## 脚本引擎 API

脚本中可通过 `env` 对象访问当前服务器的环境变量，通过 `crypto` 对象访问加密工具：

### 编码转换
- `crypto.stringToBytes(str)` / `crypto.bytesToString(bytes)`
- `crypto.bytesToBase64(bytes)` / `crypto.base64ToBytes(str)`
- `crypto.bytesToHex(bytes)` / `crypto.hexToBytes(hex)`

### 哈希函数
- `await crypto.sha256(data)` / `await crypto.sha1(data)` / `crypto.md5(data)`
- `await crypto.hmacSha256(key, data)`

### AES 加解密 (Base64 格式)
- `await crypto.aesGcmEncrypt(text, keyBase64, ivBase64?)`
- `await crypto.aesGcmDecrypt(cipherBase64, keyBase64)`
- `await crypto.aesCbcEncrypt(text, keyBase64, ivBase64?)`
- `await crypto.aesCbcDecrypt(cipherBase64, keyBase64)`

### AES 加解密 (Hex 格式)
- `await crypto.aesGcmEncryptHex(text, keyHex, ivHex?)`
- `await crypto.aesGcmDecryptHex(cipherHex, keyHex)`
- `await crypto.aesCbcEncryptHex(text, keyHex, ivHex?)`
- `await crypto.aesCbcDecryptHex(cipherHex, keyHex)`

### 其他工具
- `crypto.randomBytes(length)` / `crypto.generateKey(bits)` / `crypto.generateIv(length)`
- `crypto.xor(data, key)` / `crypto.crc32(data)`

## 开发注意事项

1. **运行环境**: Windows 11 + PowerShell
2. **前端构建命令**: `npm run build`
3. **数据存储**: 配置和脚本存储在 YAML 文件中，位于用户数据目录
4. **Tauri 通信**: 前端通过 `@tauri-apps/api` 的 `invoke` 调用 Rust 命令
5. **状态同步**: Rust 后端通过 Tauri 事件系统推送状态变化到前端

## 文档参考

- 功能需求: `docs/阶段开发文档/`
- 开发进度: `docs/阶段进度文档/`
- 开发计划: `docs/development-plan.md`
