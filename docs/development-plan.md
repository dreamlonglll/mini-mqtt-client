# Mini MQTT Client 开发计划

## 项目概述

基于 Tauri 2 + Vue 3 + TypeScript 开发的 Windows MQTT 调试客户端，提供直观的用户界面来管理 MQTT 连接、发布/订阅消息。

## 技术栈

| 层级 | 技术选型 | 说明 |
|------|----------|------|
| 前端框架 | Vue 3 + TypeScript | 响应式 UI 开发 |
| UI 组件库 | Element Plus | 功能丰富、文档完善 |
| 构建工具 | Vite 6 | 快速开发构建 |
| 桌面框架 | Tauri 2 | 轻量级跨平台桌面应用 |
| 后端语言 | Rust | 高性能、内存安全 |
| MQTT 库 | rumqttc (Rust) | 支持 MQTT 3.1.1 和 5.0 |
| 数据存储 | SQLite (rusqlite) | 结构化存储、查询方便 |
| 状态管理 | Pinia | Vue 3 官方推荐 |

## 核心功能模块

### 1. MQTT Server 管理
- 新增/编辑/删除 MQTT Server 配置
- 配置项：名称、地址、端口、协议版本(3.1.1/5.0)、用户名、密码、Client ID、Keep Alive、Clean Session
- 支持 TLS/SSL 连接配置
- 连接状态实时显示

### 2. 命令管理（每个 Server 独立）
- 保存历史发送命令
- 命令列表的增删改查
- 命令分组/分类管理
- 快速重发历史命令
- 命令模板功能

### 3. 发布/订阅功能
- **发布消息**
  - Topic 输入（支持历史记录）
  - Payload 编辑器（支持 JSON 格式化）
  - QoS 级别选择 (0/1/2)
  - Retain 标志设置
  - MQTT 5.0: 用户属性、消息过期时间
  
- **订阅管理**
  - 多 Topic 同时订阅
  - 通配符支持 (+/#)
  - QoS 级别设置
  - 订阅列表管理
  - 一键取消订阅

### 4. 消息显示
- 实时消息流展示
- 消息格式切换：JSON / HEX / 纯文本
- 消息过滤（按 Topic、时间范围）
- 消息搜索
- 消息导出功能

### 5. 界面功能
- 深色/浅色主题切换
- 响应式布局
- 多标签页支持（多连接同时管理）

## 数据库设计

### 表结构

```sql
-- MQTT Server 配置表
CREATE TABLE mqtt_servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER DEFAULT 1883,
    protocol_version TEXT DEFAULT '3.1.1',  -- '3.1.1' 或 '5.0'
    username TEXT,
    password TEXT,
    client_id TEXT,
    keep_alive INTEGER DEFAULT 60,
    clean_session INTEGER DEFAULT 1,
    use_tls INTEGER DEFAULT 0,
    ca_cert TEXT,
    client_cert TEXT,
    client_key TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 命令模板表
CREATE TABLE command_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    topic TEXT NOT NULL,
    payload TEXT,
    qos INTEGER DEFAULT 0,
    retain INTEGER DEFAULT 0,
    category TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
);

-- 历史消息表
CREATE TABLE message_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    direction TEXT NOT NULL,  -- 'publish' 或 'receive'
    topic TEXT NOT NULL,
    payload BLOB,
    qos INTEGER DEFAULT 0,
    retain INTEGER DEFAULT 0,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
);

-- 订阅记录表
CREATE TABLE subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    topic TEXT NOT NULL,
    qos INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES mqtt_servers(id) ON DELETE CASCADE
);
```

## 项目结构

```
mini-mqtt-client/
├── docs/                          # 文档目录
├── src/                           # Vue 前端源码
│   ├── assets/                    # 静态资源
│   │   └── styles/                # 全局样式
│   ├── components/                # 通用组件
│   │   ├── common/                # 公共组件
│   │   ├── layout/                # 布局组件
│   │   └── mqtt/                  # MQTT 相关组件
│   ├── composables/               # 组合式函数
│   ├── stores/                    # Pinia 状态管理
│   ├── types/                     # TypeScript 类型定义
│   ├── utils/                     # 工具函数
│   ├── views/                     # 页面视图
│   ├── App.vue                    # 根组件
│   └── main.ts                    # 入口文件
├── src-tauri/                     # Tauri Rust 后端
│   ├── src/
│   │   ├── commands/              # Tauri 命令
│   │   │   ├── mod.rs
│   │   │   ├── server.rs          # Server 管理命令
│   │   │   ├── mqtt.rs            # MQTT 连接命令
│   │   │   └── template.rs        # 模板管理命令
│   │   ├── db/                    # 数据库操作
│   │   │   ├── mod.rs
│   │   │   └── models.rs          # 数据模型
│   │   ├── mqtt/                  # MQTT 客户端封装
│   │   │   ├── mod.rs
│   │   │   └── client.rs
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── index.html
├── package.json
├── vite.config.ts
└── tsconfig.json
```

## 开发阶段划分

### 阶段一：基础架构搭建
1. 配置 Element Plus 组件库
2. 配置 Pinia 状态管理
3. 设计并实现基础布局组件
4. 配置深浅色主题切换
5. Rust 端添加 SQLite 和 rumqttc 依赖
6. 实现数据库初始化和迁移

### 阶段二：Server 管理功能
1. 实现 Server 配置的 CRUD 接口 (Rust)
2. 开发 Server 列表界面
3. 开发 Server 配置表单（新增/编辑）
4. 实现配置数据持久化

### 阶段三：MQTT 连接功能
1. 封装 MQTT 客户端 (Rust rumqttc)
2. 实现连接/断开功能
3. 实现连接状态同步到前端
4. 处理连接错误和重连逻辑

### 阶段四：发布/订阅功能
1. 实现消息发布接口
2. 实现 Topic 订阅/取消订阅
3. 实现消息接收和推送到前端
4. 开发发布消息界面
5. 开发订阅管理界面

### 阶段五：消息展示功能
1. 实现消息历史存储
2. 开发消息列表组件
3. 实现 JSON/HEX/纯文本格式切换
4. 实现消息过滤和搜索
5. 实现消息导出

### 阶段六：命令模板功能
1. 实现命令模板 CRUD 接口
2. 开发命令模板列表界面
3. 开发模板编辑表单
4. 实现快速发送功能

### 阶段七：优化与完善
1. 界面美化和交互优化
2. 错误处理完善
3. 性能优化
4. 测试和 Bug 修复

## 依赖清单

### 前端依赖 (npm)
```json
{
  "dependencies": {
    "vue": "^3.5.13",
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-opener": "^2",
    "element-plus": "^2.9.1",
    "pinia": "^2.3.0",
    "@element-plus/icons-vue": "^2.3.1"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "^5.2.1",
    "typescript": "~5.6.2",
    "vite": "^6.0.3",
    "vue-tsc": "^2.1.10",
    "@tauri-apps/cli": "^2",
    "unplugin-auto-import": "^0.18.6",
    "unplugin-vue-components": "^0.27.5",
    "sass": "^1.83.0"
  }
}
```

### Rust 依赖 (Cargo.toml)
```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rumqttc = "0.24"
rusqlite = { version = "0.32", features = ["bundled"] }
tokio = { version = "1", features = ["full"] }
thiserror = "2.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.11", features = ["v4"] }
```

## 界面设计要点

### 主界面布局
```
+------------------+------------------------+
|                  |                        |
|   Server 列表    |     消息区域           |
|   (左侧边栏)     |     (中央主区域)       |
|                  |                        |
|   - Server 1     |   +----------------+   |
|   - Server 2     |   | 订阅 Topic 列表 |   |
|   - ...          |   +----------------+   |
|                  |   | 消息列表        |   |
|   [+新增Server]  |   |                |   |
|                  |   +----------------+   |
|                  |   | 发布消息区域    |   |
|                  |   +----------------+   |
+------------------+------------------------+
|                  命令模板快捷栏           |
+------------------------------------------+
```

### 配色方案
- **浅色主题**: Element Plus 默认主题
- **深色主题**: Element Plus 暗黑主题 + 自定义调整
- **强调色**: #409EFF (Element Plus 主色)

## 注意事项

1. **MQTT 连接管理**: 每个 Server 配置独立维护连接状态，支持多个连接同时存在
2. **消息性能**: 消息列表需要虚拟滚动，避免大量消息导致性能问题
3. **数据安全**: 密码等敏感信息在存储时需要考虑加密
4. **错误处理**: MQTT 连接失败、网络异常等情况需要友好提示
5. **状态同步**: Rust 后端状态变化需要及时推送到前端

---

*文档版本: 1.0*
*创建日期: 2026-01-15*
