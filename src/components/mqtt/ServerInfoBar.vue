<template>
  <div class="server-info-bar app-card">
    <template v-if="activeServer">
      <!-- 左侧：Server 信息 -->
      <div class="server-info">
        <span class="status-indicator" :class="activeServer.status" />
        <div class="server-details">
          <span class="server-name">{{ activeServer.server.name }}</span>
          <span class="server-address">
            {{ formatServerAddress(activeServer.server) }}
          </span>
        </div>
        <el-tag
          :type="statusTagType"
          size="small"
          effect="plain"
        >
          {{ statusText }}
        </el-tag>
      </div>

      <!-- 中间：协议和配置信息 -->
      <div class="server-config">
        <el-tag size="small" type="info" effect="plain">
          MQTT {{ activeServer.server.protocol_version }}
        </el-tag>
        <el-tag v-if="activeServer.server.use_tls" size="small" type="success" effect="plain">
          TLS
        </el-tag>
        <el-tag size="small" effect="plain">
          Keep Alive: {{ activeServer.server.keep_alive }}s
        </el-tag>
      </div>

      <!-- 右侧：操作按钮 -->
      <div class="server-actions">
        <el-button
          v-if="activeServer.status === 'disconnected' || activeServer.status === 'error'"
          type="primary"
          size="small"
          :icon="Connection"
          @click="handleConnect"
        >
          连接
        </el-button>
        <el-button
          v-else-if="activeServer.status === 'connected'"
          type="danger"
          size="small"
          plain
          :icon="SwitchButton"
          @click="handleDisconnect"
        >
          断开
        </el-button>
        <el-button v-else type="warning" size="small" :loading="true" disabled>
          连接中
        </el-button>
        <el-button :icon="Setting" size="small" @click="handleSettings" />
      </div>
    </template>

    <template v-else>
      <div class="no-server">
        <el-icon><Warning /></el-icon>
        <span>请在左侧选择或创建一个 Server</span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  Connection,
  SwitchButton,
  Setting,
  Warning,
} from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/server";
import type { MqttServer } from "@/types/mqtt";

const serverStore = useServerStore();

// 格式化服务器地址为 协议://host:port 格式
const formatServerAddress = (server: MqttServer): string => {
  const protocol = server.use_tls ? "mqtts" : "mqtt";
  return `${protocol}://${server.host}:${server.port}`;
};

const activeServer = computed(() => serverStore.activeServer);

const statusText = computed(() => {
  switch (activeServer.value?.status) {
    case "connected":
      return "已连接";
    case "connecting":
      return "连接中...";
    case "error":
      return "连接错误";
    default:
      return "未连接";
  }
});

const statusTagType = computed(() => {
  switch (activeServer.value?.status) {
    case "connected":
      return "success";
    case "connecting":
      return "warning";
    case "error":
      return "danger";
    default:
      return "info";
  }
});

const handleConnect = () => {
  const server = activeServer.value;
  if (server?.server.id) {
    const id = server.server.id;
    serverStore.setConnectionStatus(id, "connecting");
    // 模拟连接
    setTimeout(() => {
      serverStore.setConnectionStatus(id, "connected");
    }, 1500);
  }
};

const handleDisconnect = () => {
  const server = activeServer.value;
  if (server?.server.id) {
    serverStore.setConnectionStatus(server.server.id, "disconnected");
  }
};

const handleSettings = () => {
  console.log("Open settings");
};
</script>

<style scoped lang="scss">
.server-info-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  gap: 16px;
}

.server-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.server-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.server-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.server-address {
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-secondary);
}

.server-config {
  display: flex;
  align-items: center;
  gap: 8px;
}

.server-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.no-server {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--app-text-secondary);
  font-size: 14px;

  .el-icon {
    font-size: 18px;
  }
}
</style>
