<template>
  <div class="sidebar">
    <!-- Logo 区域 -->
    <div class="sidebar-header">
      <div class="logo">
        <div class="logo-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path
              d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"
            />
          </svg>
        </div>
        <span class="logo-text">MQTT Client</span>
      </div>
    </div>

    <!-- Server 列表 -->
    <div class="sidebar-content">
      <div class="section">
        <div class="section-header">
          <span class="section-title">Server</span>
          <el-button
            type="primary"
            size="small"
            :icon="Plus"
            circle
            @click="handleAddServer"
          />
        </div>

        <div class="server-list">
          <div
            v-for="serverState in serverStore.servers"
            :key="serverState.server.id"
            class="server-item"
            :class="{ active: serverState.server.id === serverStore.activeServerId }"
            @click="handleSelectServer(serverState.server.id)"
          >
            <span
              class="status-indicator"
              :class="serverState.status"
            />
            <div class="server-info">
              <span class="server-name text-ellipsis">{{ serverState.server.name }}</span>
              <span class="server-host text-ellipsis">
                {{ serverState.server.host }}:{{ serverState.server.port }}
              </span>
            </div>
            <el-dropdown trigger="click" @command="handleServerAction">
              <el-button :icon="MoreFilled" text size="small" class="more-btn" />
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item :command="{ action: 'edit', id: serverState.server.id }">
                    <el-icon><Edit /></el-icon>
                    <span>编辑</span>
                  </el-dropdown-item>
                  <el-dropdown-item :command="{ action: 'duplicate', id: serverState.server.id }">
                    <el-icon><CopyDocument /></el-icon>
                    <span>复制</span>
                  </el-dropdown-item>
                  <el-dropdown-item
                    :command="{ action: 'delete', id: serverState.server.id }"
                    divided
                  >
                    <el-icon><Delete /></el-icon>
                    <span style="color: var(--el-color-danger)">删除</span>
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>

          <!-- 空状态 -->
          <div v-if="serverStore.servers.length === 0" class="empty-state">
            <el-empty description="暂无 Server" :image-size="60">
              <el-button type="primary" size="small" @click="handleAddServer">
                添加 Server
              </el-button>
            </el-empty>
          </div>
        </div>
      </div>
    </div>

    <!-- 底部 -->
    <div class="sidebar-footer">
      <el-button text @click="appStore.toggleTheme">
        <el-icon>
          <Moon v-if="appStore.theme === 'light'" />
          <Sunny v-else />
        </el-icon>
        <span>{{ appStore.theme === 'light' ? '深色模式' : '浅色模式' }}</span>
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import {
  Plus,
  MoreFilled,
  Edit,
  Delete,
  CopyDocument,
  Moon,
  Sunny,
} from "@element-plus/icons-vue";
import { useAppStore } from "@/stores/app";
import { useServerStore } from "@/stores/server";

const appStore = useAppStore();
const serverStore = useServerStore();

const handleAddServer = () => {
  // TODO: 打开添加 Server 对话框
  console.log("Add server");
};

const handleSelectServer = (id: number) => {
  serverStore.setActiveServer(id);
};

interface ServerAction {
  action: "edit" | "duplicate" | "delete";
  id: number;
}

const handleServerAction = (command: ServerAction) => {
  switch (command.action) {
    case "edit":
      // TODO: 打开编辑对话框
      console.log("Edit server:", command.id);
      break;
    case "duplicate":
      // TODO: 复制 Server
      console.log("Duplicate server:", command.id);
      break;
    case "delete":
      serverStore.removeServer(command.id);
      break;
  }
};
</script>

<style scoped lang="scss">
.sidebar {
  height: 100%;
  background-color: var(--sidebar-bg);
  border-right: 1px solid var(--app-border-color);
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 16px;
  border-bottom: 1px solid var(--app-border-color);
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
}

.logo-icon {
  width: 32px;
  height: 32px;
  color: var(--primary-color);

  svg {
    width: 100%;
    height: 100%;
  }
}

.logo-text {
  font-size: 18px;
  font-weight: 600;
  color: var(--app-text-color);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.section {
  margin-bottom: 16px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding: 0 4px;
}

.section-title {
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--app-text-secondary);
}

.server-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.server-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);

    .more-btn {
      opacity: 1;
    }
  }

  &.active {
    background-color: var(--sidebar-active);
  }
}

.server-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.server-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--app-text-color);
}

.server-host {
  font-size: 12px;
  color: var(--app-text-secondary);
}

.more-btn {
  opacity: 0;
  transition: opacity 0.2s ease;
}

.empty-state {
  padding: 24px 0;
}

.sidebar-footer {
  padding: 12px 16px;
  border-top: 1px solid var(--app-border-color);
}
</style>
