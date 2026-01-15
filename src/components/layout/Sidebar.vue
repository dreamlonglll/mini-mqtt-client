<template>
  <div class="sidebar">
    <!-- Logo 区域 -->
    <div class="sidebar-header">
      <div class="logo">
        <div class="logo-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
          </svg>
        </div>
        <span class="logo-text">MQTT Client</span>
      </div>
    </div>

    <div class="sidebar-content">
      <!-- Server 列表区域 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">SERVER</span>
          <el-button type="primary" size="small" :icon="Plus" circle @click="handleAddServer" />
        </div>

        <div class="server-list">
          <div
            v-for="serverState in serverStore.servers"
            :key="serverState.server.id"
            class="server-item"
            :class="{ active: serverState.server.id === serverStore.activeServerId }"
            @click="handleSelectServer(serverState.server.id!)"
          >
            <span class="status-indicator" :class="serverState.status" />
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
                    <span>编辑</span>
                  </el-dropdown-item>
                  <el-dropdown-item command="duplicate">
                    <el-icon><CopyDocument /></el-icon>
                    <span>复制</span>
                  </el-dropdown-item>
                  <el-dropdown-item command="delete" divided>
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

      <!-- 分隔线 -->
      <el-divider v-if="serverStore.activeServer" />

      <!-- 订阅列表区域 -->
      <div v-if="serverStore.activeServer" class="section">
        <div class="section-header">
          <span class="section-title">订阅</span>
          <el-button type="primary" size="small" :icon="Plus" circle @click="handleAddSubscription" />
        </div>

        <div class="subscription-list">
          <div
            v-for="sub in subscriptions"
            :key="sub.id"
            class="subscription-item"
          >
            <div class="sub-main">
              <span class="sub-topic text-ellipsis">{{ sub.topic }}</span>
              <el-tag size="small" effect="plain" type="info">Q{{ sub.qos }}</el-tag>
            </div>
            <div class="sub-actions">
              <el-button text size="small" :icon="Edit" @click="handleEditSubscription(sub)" />
              <el-button text size="small" type="danger" :icon="Close" @click="handleDeleteSubscription(sub.id)" />
            </div>
          </div>

          <div v-if="subscriptions.length === 0" class="empty-hint">
            点击 + 添加订阅
          </div>
        </div>
      </div>
    </div>

    <!-- 底部 -->
    <div class="sidebar-footer">
      <el-button text @click="appStore.toggleTheme" class="theme-btn">
        <el-icon>
          <Moon v-if="appStore.theme === 'light'" />
          <Sunny v-else />
        </el-icon>
        <span>{{ appStore.theme === 'light' ? '深色模式' : '浅色模式' }}</span>
      </el-button>
    </div>

    <!-- Server 表单对话框 -->
    <ServerFormDialog
      v-model:visible="showServerDialog"
      :server="editingServer"
      @saved="handleServerSaved"
    />

    <!-- 订阅对话框 -->
    <el-dialog
      v-model="showSubDialog"
      :title="isEditingSub ? '编辑订阅' : '添加订阅'"
      width="420px"
      :close-on-click-modal="false"
    >
      <el-form :model="subFormData" label-width="80px">
        <el-form-item label="Topic">
          <el-input v-model="subFormData.topic" placeholder="例如: sensor/+/temperature" />
        </el-form-item>
        <el-form-item label="QoS">
          <el-radio-group v-model="subFormData.qos">
            <el-radio-button :value="0">QoS 0</el-radio-button>
            <el-radio-button :value="1">QoS 1</el-radio-button>
            <el-radio-button :value="2">QoS 2</el-radio-button>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showSubDialog = false">取消</el-button>
        <el-button type="primary" @click="handleConfirmSubscription">
          {{ isEditingSub ? '保存' : '订阅' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from "vue";
import {
  Plus,
  MoreFilled,
  Edit,
  Delete,
  Close,
  CopyDocument,
  Moon,
  Sunny,
} from "@element-plus/icons-vue";
import { useAppStore } from "@/stores/app";
import { useServerStore } from "@/stores/server";
import { ElMessage, ElMessageBox } from "element-plus";
import ServerFormDialog from "@/components/mqtt/ServerFormDialog.vue";
import type { MqttServer } from "@/types/mqtt";

const appStore = useAppStore();

// 格式化服务器地址为 协议://host:port 格式
const formatServerAddress = (server: MqttServer): string => {
  const protocol = server.use_tls ? "mqtts" : "mqtt";
  return `${protocol}://${server.host}:${server.port}`;
};
const serverStore = useServerStore();

// 初始化加载 Server 列表
onMounted(() => {
  serverStore.fetchServers();
});

// ===== Server 相关 =====
const showServerDialog = ref(false);
const editingServer = ref<MqttServer | null>(null);

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
      ElMessage.success("复制成功");
      break;
    case "delete":
      try {
        await ElMessageBox.confirm(
          `确定要删除 "${server.name}" 吗？`,
          "确认删除",
          {
            confirmButtonText: "删除",
            cancelButtonText: "取消",
            type: "warning",
          }
        );
        await serverStore.removeServer(server.id!);
        ElMessage.success("删除成功");
      } catch {
        // 用户取消
      }
      break;
  }
};

const handleServerSaved = () => {
  // 对话框会自动关闭
};

// ===== 订阅相关 =====
interface Subscription {
  id: number;
  topic: string;
  qos: number;
}

const subscriptions = ref<Subscription[]>([]);
const showSubDialog = ref(false);
const isEditingSub = ref(false);
const editingSubId = ref<number | null>(null);

const subFormData = reactive({
  topic: "",
  qos: 0,
});

let nextSubId = 1;

const handleAddSubscription = () => {
  isEditingSub.value = false;
  editingSubId.value = null;
  subFormData.topic = "";
  subFormData.qos = 0;
  showSubDialog.value = true;
};

const handleEditSubscription = (sub: Subscription) => {
  isEditingSub.value = true;
  editingSubId.value = sub.id;
  subFormData.topic = sub.topic;
  subFormData.qos = sub.qos;
  showSubDialog.value = true;
};

const handleDeleteSubscription = async (id: number) => {
  try {
    await ElMessageBox.confirm("确定要取消此订阅吗？", "确认", {
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      type: "warning",
    });
    const index = subscriptions.value.findIndex((s) => s.id === id);
    if (index !== -1) {
      subscriptions.value.splice(index, 1);
      ElMessage.success("订阅已取消");
    }
  } catch {
    // 用户取消
  }
};

const handleConfirmSubscription = () => {
  if (!subFormData.topic.trim()) {
    ElMessage.warning("请输入 Topic");
    return;
  }

  if (isEditingSub.value && editingSubId.value !== null) {
    const sub = subscriptions.value.find((s) => s.id === editingSubId.value);
    if (sub) {
      sub.topic = subFormData.topic;
      sub.qos = subFormData.qos;
      ElMessage.success("订阅已更新");
    }
  } else {
    subscriptions.value.push({
      id: nextSubId++,
      topic: subFormData.topic,
      qos: subFormData.qos,
    });
    ElMessage.success("订阅成功");
  }
  showSubDialog.value = false;
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
  width: 28px;
  height: 28px;
  color: var(--primary-color);

  svg {
    width: 100%;
    height: 100%;
  }
}

.logo-text {
  font-size: 16px;
  font-weight: 600;
  color: var(--app-text-color);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.section {
  margin-bottom: 8px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  padding: 0 4px;
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--app-text-secondary);
}

.server-list,
.subscription-list {
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

.subscription-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  border-radius: 6px;
  transition: background-color 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);

    .sub-actions {
      opacity: 1;
    }
  }
}

.sub-main {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
}

.sub-topic {
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
}

.sub-actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.empty-state {
  padding: 16px 0;
}

.empty-hint {
  text-align: center;
  padding: 12px;
  font-size: 12px;
  color: var(--app-text-secondary);
}

.sidebar-footer {
  padding: 8px 12px;
  border-top: 1px solid var(--app-border-color);
}

.theme-btn {
  width: 100%;
  justify-content: flex-start;
}

:deep(.el-divider) {
  margin: 8px 0;
}
</style>
