<template>
  <div class="command-bar">
    <div class="command-bar-content">
      <el-icon class="command-icon"><Histogram /></el-icon>
      <span class="command-label">快捷命令</span>
      <el-divider direction="vertical" />
      
      <div class="command-list">
        <!-- 常用模板快捷按钮 -->
        <el-button
          v-for="template in frequentTemplates"
          :key="template.id"
          size="small"
          @click="handleQuickSend(template)"
        >
          {{ template.name }}
        </el-button>
        
        <!-- 无模板时显示添加按钮 -->
        <el-button
          v-if="frequentTemplates.length === 0"
          size="small"
          type="primary"
          plain
          :icon="Plus"
          @click="handleOpenTemplates"
        >
          添加命令模板
        </el-button>
      </div>
    </div>
    
    <div class="command-bar-actions">
      <el-button size="small" :icon="FolderOpened" text @click="handleOpenTemplates">
        管理模板
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch } from "vue";
import { Histogram, Plus, FolderOpened } from "@element-plus/icons-vue";
import { useTemplateStore, type CommandTemplate } from "@/stores/template";
import { useServerStore } from "@/stores/server";
import { useAppStore } from "@/stores/app";
import { ElMessage } from "element-plus";

const templateStore = useTemplateStore();
const serverStore = useServerStore();
const appStore = useAppStore();

// 常用模板（前5个）
const frequentTemplates = computed(() => templateStore.frequentTemplates);

// 监听服务器变化，加载模板
watch(
  () => serverStore.activeServerId,
  (serverId) => {
    if (serverId) {
      templateStore.loadTemplates(serverId);
    }
  },
  { immediate: true }
);

// 快速发送模板
async function handleQuickSend(template: CommandTemplate) {
  try {
    const used = await templateStore.useTemplate(template.id!);
    // 复制到发布面板
    appStore.setCopyToPublish({
      topic: used.topic,
      payload: used.payload,
      qos: used.qos,
      retain: used.retain,
      payloadType: used.payload_type,
    });
    ElMessage.success(`已加载: ${template.name}`);
  } catch (error) {
    ElMessage.error("加载模板失败");
  }
}

const emit = defineEmits<{
  openTemplates: []
}>();

// 打开模板管理
function handleOpenTemplates() {
  emit("openTemplates");
}
</script>

<style scoped lang="scss">
.command-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 16px;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-top: 1px solid var(--app-border-color);
}

.command-bar-content {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.command-icon {
  color: var(--primary-color);
}

.command-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-secondary);
  white-space: nowrap;
}

.command-list {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  flex: 1;
  min-width: 0;

  &::-webkit-scrollbar {
    height: 4px;
  }
}

.command-bar-actions {
  flex-shrink: 0;
  margin-left: 16px;
}
</style>
