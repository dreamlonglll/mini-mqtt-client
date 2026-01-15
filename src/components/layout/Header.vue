<template>
  <div class="header">
    <!-- 左侧：Server 信息 -->
    <div class="header-left">
      <template v-if="activeServer">
        <span class="status-indicator" :class="connectionStatus" />
        <div class="server-details">
          <span class="server-name">{{ activeServer.server.name }}</span>
          <span class="server-address">
            {{ formatServerAddress(activeServer.server) }}
          </span>
        </div>
        <el-tag :type="statusTagType" size="small" effect="plain">
          {{ statusText }}
        </el-tag>
      </template>
      <span v-else class="welcome-text">欢迎使用 Mini MQTT Client</span>
    </div>

    <!-- 中间：协议和配置信息 -->
    <div v-if="activeServer" class="header-center">
      <el-tag size="small" type="info" effect="plain">
        MQTT {{ activeServer.server.protocol_version }}
      </el-tag>
      <el-tag
        v-if="activeServer.server.use_tls"
        size="small"
        type="success"
        effect="plain"
      >
        TLS
      </el-tag>
      <el-tag size="small" effect="plain">
        Keep Alive: {{ activeServer.server.keep_alive }}s
      </el-tag>
    </div>

    <!-- 右侧：操作按钮 -->
    <div class="header-right">
      <template v-if="activeServer">
        <el-button
          v-if="connectionStatus === 'disconnected' || connectionStatus === 'error'"
          type="primary"
          size="small"
          :icon="Connection"
          :loading="connecting"
          @click="handleConnect"
        >
          连接
        </el-button>
        <el-button
          v-else-if="connectionStatus === 'connected'"
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
      </template>
      <el-tooltip content="设置" placement="bottom">
        <el-button :icon="Setting" circle size="small" @click="handleSettings" />
      </el-tooltip>
    </div>

    <!-- 错误提示 -->
    <el-alert
      v-if="connectionError"
      :title="connectionError"
      type="error"
      show-icon
      :closable="false"
      class="error-alert"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Connection,
  SwitchButton,
  Setting,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useServerStore } from "@/stores/server";
import { useMqttStore } from "@/stores/mqtt";
import { useSubscriptionStore } from "@/stores/subscription";
import type { MqttServer } from "@/types/mqtt";

const serverStore = useServerStore();
const mqttStore = useMqttStore();
const subscriptionStore = useSubscriptionStore();
const connecting = ref(false);

// 格式化服务器地址为 协议://host:port 格式
const formatServerAddress = (server: MqttServer): string => {
  const protocol = server.use_tls ? "mqtts" : "mqtt";
  return `${protocol}://${server.host}:${server.port}`;
};

const activeServer = computed(() => serverStore.activeServer);

const connectionStatus = computed(() => {
  if (!activeServer.value?.server.id) return "disconnected";
  return mqttStore.getConnectionStatus(activeServer.value.server.id);
});

const connectionError = computed(() => {
  if (!activeServer.value?.server.id) return undefined;
  return mqttStore.getConnectionError(activeServer.value.server.id);
});

const statusText = computed(() => {
  switch (connectionStatus.value) {
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
  switch (connectionStatus.value) {
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

// 监听连接状态变化，自动订阅活跃的订阅
watch(connectionStatus, async (newStatus, oldStatus) => {
  if (newStatus === "connected" && oldStatus !== "connected") {
    const serverId = activeServer.value?.server.id;
    if (!serverId) return;

    // 获取所有活跃的订阅
    const subscriptions = subscriptionStore.getSubscriptionsByServer(serverId);
    const activeSubscriptions = subscriptions.filter((sub) => sub.is_active);

    // 自动订阅
    for (const sub of activeSubscriptions) {
      try {
        await mqttStore.subscribe(serverId, sub.topic, sub.qos as 0 | 1 | 2);
      } catch (e) {
        console.error(`自动订阅失败: ${sub.topic}`, e);
      }
    }

    if (activeSubscriptions.length > 0) {
      // ElMessage.success(`已自动订阅 ${activeSubscriptions.length} 个主题`);
    }
  }
});

const handleConnect = async () => {
  const server = activeServer.value;
  if (!server?.server.id) return;

  connecting.value = true;
  try {
    await mqttStore.connect(server.server.id);
  } catch (e) {
    ElMessage.error(`连接失败: ${e}`);
  } finally {
    connecting.value = false;
  }
};

const handleDisconnect = async () => {
  const server = activeServer.value;
  if (!server?.server.id) return;

  try {
    await mqttStore.disconnect(server.server.id);
  } catch (e) {
    ElMessage.error(`断开失败: ${e}`);
  }
};

const handleSettings = () => {
  console.log("Open settings");
};
</script>

<style scoped lang="scss">
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-bottom: 1px solid var(--app-border-color);
  gap: 16px;
  flex-wrap: wrap;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 200px;
}

.server-details {
  display: flex;
  flex-direction: column;
  gap: 0px;
}

.server-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
  line-height: 1.3;
}

.server-address {
  font-size: 11px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-secondary);
  line-height: 1.3;
}

.header-center {
  display: flex;
  align-items: center;
  gap: 8px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.welcome-text {
  font-size: 14px;
  color: var(--app-text-secondary);
}

.error-alert {
  width: 100%;
  margin-top: 8px;
}

.status-indicator {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;

  &.connected {
    background-color: var(--status-connected);
    box-shadow: 0 0 6px var(--status-connected);
  }

  &.connecting {
    background-color: var(--status-connecting);
    animation: pulse 1.5s infinite;
  }

  &.disconnected {
    background-color: var(--status-disconnected);
  }

  &.error {
    background-color: var(--status-error);
  }
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}
</style>
