<template>
  <div class="main-content">
    <!-- 上部：Server 信息和状态栏 -->
    <ServerInfoBar class="server-info-bar" />

    <!-- 消息区域 -->
    <div class="content-body">
      <!-- 消息列表 -->
      <MessageList class="message-list" />

      <!-- 发布消息 -->
      <PublishPanel 
        class="publish-panel" 
        @save-template="handleSaveTemplate" 
        @open-templates="handleOpenTemplates"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import ServerInfoBar from "./ServerInfoBar.vue";
import MessageList from "./MessageList.vue";
import PublishPanel from "./PublishPanel.vue";

interface SaveTemplateData {
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
  payloadType: string;
}

const emit = defineEmits<{
  saveTemplate: [data: SaveTemplateData]
  openTemplates: []
}>();

function handleSaveTemplate(data: SaveTemplateData) {
  emit('saveTemplate', data);
}

function handleOpenTemplates() {
  emit('openTemplates');
}
</script>

<style scoped lang="scss">
.main-content {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 12px;
  padding: 12px 16px;
  overflow: hidden;
}

.server-info-bar {
  flex-shrink: 0;
}

.content-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  overflow: hidden;
}

.message-list {
  flex: 1;
  min-height: 200px;
}

.publish-panel {
  flex-shrink: 0;
}
</style>
