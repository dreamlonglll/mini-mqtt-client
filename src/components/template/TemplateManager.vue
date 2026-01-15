<template>
  <div class="template-manager">
    <div class="manager-header">
      <div class="header-info">
        <h2 class="title">
          <el-icon><Files /></el-icon>
          命令模板
        </h2>
        <span class="subtitle">管理和快速发送MQTT消息模板</span>
      </div>
      <el-button text @click="$emit('close')">
        <el-icon><Close /></el-icon>
      </el-button>
    </div>

    <div class="manager-content">
      <!-- 左侧：模板列表 -->
      <div class="main-panel">
        <TemplateList
          v-if="serverId"
          :server-id="serverId"
          @use="handleUseTemplate"
        />
        <el-empty v-else description="请先选择一个服务器" :image-size="100" />
      </div>

      <!-- 右侧：快速发送面板 -->
      <div class="side-panel">
        <QuickSendPanel
          v-if="serverId"
          :server-id="serverId"
          @send="handleUseTemplate"
          @manage="scrollToList"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Files, Close } from '@element-plus/icons-vue'
import TemplateList from './TemplateList.vue'
import QuickSendPanel from './QuickSendPanel.vue'
import type { CommandTemplate } from '@/stores/template'

defineProps<{
  serverId: number | null
}>()

const emit = defineEmits<{
  close: []
  use: [template: CommandTemplate]
}>()

function handleUseTemplate(template: CommandTemplate) {
  emit('use', template)
}

function scrollToList() {
  // 滚动到列表顶部
  const mainPanel = document.querySelector('.main-panel')
  if (mainPanel) {
    mainPanel.scrollTo({ top: 0, behavior: 'smooth' })
  }
}
</script>

<style scoped lang="scss">
.template-manager {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--app-bg-color);
}

.manager-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--app-border-color);
  background-color: var(--card-bg);
  flex-shrink: 0;
}

.header-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--app-text-color);
}

.subtitle {
  font-size: 13px;
  color: var(--app-text-secondary);
}

.manager-content {
  flex: 1;
  display: flex;
  gap: 16px;
  padding: 16px 20px;
  overflow: hidden;
}

.main-panel {
  flex: 1;
  min-width: 0;
  overflow: auto;
}

.side-panel {
  width: 320px;
  flex-shrink: 0;
}
</style>
