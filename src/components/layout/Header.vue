<template>
  <div class="header">
    <div class="header-left">
      <template v-if="activeServer">
        <span
          class="status-indicator"
          :class="activeServer.status"
        />
        <span class="server-name">{{ activeServer.server.name }}</span>
        <el-tag
          v-if="activeServer.status === 'connected'"
          type="success"
          size="small"
          effect="plain"
        >
          已连接
        </el-tag>
        <el-tag
          v-else-if="activeServer.status === 'connecting'"
          type="warning"
          size="small"
          effect="plain"
        >
          连接中...
        </el-tag>
        <el-tag
          v-else-if="activeServer.status === 'error'"
          type="danger"
          size="small"
          effect="plain"
        >
          连接错误
        </el-tag>
        <el-tag v-else size="small" effect="plain" type="info">
          未连接
        </el-tag>
      </template>
      <span v-else class="no-server">请选择或创建一个 Server</span>
    </div>

    <div class="header-right">
      <template v-if="activeServer">
        <el-button
          v-if="activeServer.status === 'disconnected' || activeServer.status === 'error'"
          type="primary"
          :icon="Connection"
          @click="handleConnect"
        >
          连接
        </el-button>
        <el-button
          v-else-if="activeServer.status === 'connected'"
          type="danger"
          plain
          :icon="SwitchButton"
          @click="handleDisconnect"
        >
          断开
        </el-button>
        <el-button
          v-else
          type="warning"
          :loading="true"
          disabled
        >
          连接中
        </el-button>

        <el-divider direction="vertical" />

        <el-button :icon="Setting" circle @click="handleSettings" />
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { Connection, SwitchButton, Setting } from "@element-plus/icons-vue";
import { useServerStore } from "@/stores/server";

const serverStore = useServerStore();

const activeServer = computed(() => serverStore.activeServer);

const handleConnect = () => {
  // TODO: 调用 Tauri 命令连接
  if (activeServer.value) {
    serverStore.setConnectionStatus(activeServer.value.server.id, "connecting");
    // 模拟连接
    setTimeout(() => {
      serverStore.setConnectionStatus(
        activeServer.value!.server.id,
        "connected"
      );
    }, 1500);
  }
};

const handleDisconnect = () => {
  // TODO: 调用 Tauri 命令断开连接
  if (activeServer.value) {
    serverStore.setConnectionStatus(
      activeServer.value.server.id,
      "disconnected"
    );
  }
};

const handleSettings = () => {
  // TODO: 打开设置对话框
  console.log("Open settings");
};
</script>

<style scoped lang="scss">
.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 20px;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-bottom: 1px solid var(--app-border-color);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.server-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--app-text-color);
}

.no-server {
  font-size: 14px;
  color: var(--app-text-secondary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
