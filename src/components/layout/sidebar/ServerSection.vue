<template>
  <SidebarSection :title="$t('sidebar.server')">
    <template #action>
      <el-button type="primary" size="small" :icon="Plus" circle @click="handleAddServer" />
    </template>

    <div class="server-list">
      <div
        v-for="serverState in serverStore.servers"
        :key="serverState.server.id"
        class="server-item"
        :class="{ active: serverState.server.id === serverStore.activeServerId }"
        @click="handleSelectServer(serverState.server.id!)"
      >
        <span class="status-indicator" :class="mqttStore.getConnectionStatus(serverState.server.id!)" />
        <div class="server-info">
          <span class="server-name text-ellipsis">{{ serverState.server.name }}</span>
          <span class="server-host text-ellipsis">
            {{ formatServerAddress(serverState.server) }}
          </span>
        </div>
        <el-dropdown trigger="click" @command="(cmd: string) => handleServerAction(cmd, serverState.server)">
          <el-button :icon="MoreFilled" text size="small" class="more-btn" @click.stop />
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="edit">
                <el-icon><Edit /></el-icon>
                <span>{{ $t('sidebar.actions.edit') }}</span>
              </el-dropdown-item>
              <el-dropdown-item command="duplicate">
                <el-icon><CopyDocument /></el-icon>
                <span>{{ $t('sidebar.actions.duplicate') }}</span>
              </el-dropdown-item>
              <el-dropdown-item command="delete" divided>
                <el-icon><Delete /></el-icon>
                <span style="color: var(--el-color-danger)">{{ $t('sidebar.actions.delete') }}</span>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>

      <!-- 空状态 -->
      <div v-if="serverStore.servers.length === 0" class="empty-state">
        <el-empty :description="$t('sidebar.noServer')" :image-size="60">
          <el-button type="primary" size="small" @click="handleAddServer">
            {{ $t('sidebar.addServer') }}
          </el-button>
        </el-empty>
      </div>
    </div>
  </SidebarSection>

  <!-- Server 表单对话框 -->
  <ServerFormDialog v-model:visible="showServerDialog" :server="editingServer" />
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, MoreFilled, Edit, Delete, CopyDocument } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useServerStore } from "@/stores/server";
import { useMqttStore } from "@/stores/mqtt";
import ServerFormDialog from "@/components/mqtt/ServerFormDialog.vue";
import SidebarSection from "./SidebarSection.vue";
import type { MqttServer } from "@/types/mqtt";

const { t } = useI18n();
const serverStore = useServerStore();
const mqttStore = useMqttStore();

const showServerDialog = ref(false);
const editingServer = ref<MqttServer | null>(null);

onMounted(() => {
  serverStore.fetchServers();
});

// 格式化服务器地址为 协议://host:port 格式
const formatServerAddress = (server: MqttServer): string => {
  const protocol = server.use_tls ? "mqtts" : "mqtt";
  return `${protocol}://${server.host}:${server.port}`;
};

const handleAddServer = () => {
  editingServer.value = null;
  showServerDialog.value = true;
};

const handleSelectServer = (id: number) => {
  serverStore.setActiveServer(id);
};

const handleServerAction = async (command: string, server: MqttServer) => {
  switch (command) {
    case "edit":
      editingServer.value = server;
      showServerDialog.value = true;
      break;
    case "duplicate":
      await serverStore.duplicateServer(server.id!);
      ElMessage.success(t('server.duplicateSuccess'));
      break;
    case "delete":
      try {
        await ElMessageBox.confirm(
          t('sidebar.deleteServerConfirm', { name: server.name }),
          t('common.confirm'),
          {
            confirmButtonText: t('common.delete'),
            cancelButtonText: t('common.cancel'),
            type: "warning",
          }
        );
        await serverStore.removeServer(server.id!);
        ElMessage.success(t('server.deleteSuccess'));
      } catch {
        // 用户取消
      }
      break;
  }
};
</script>

<style scoped lang="scss">
.server-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.server-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
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
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-color);
}

.server-host {
  font-size: 11px;
  color: var(--app-text-secondary);
}

.more-btn {
  opacity: 0;
  transition: opacity 0.2s ease;
}

.empty-state {
  padding: 16px 0;
}
</style>
