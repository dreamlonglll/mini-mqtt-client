<template>
  <div class="main-content">
    <!-- 消息列表 -->
    <MessageList class="message-list" />

    <!-- 发布消息 -->
    <PublishPanel 
      class="publish-panel" 
      @save-template="handleSaveTemplate" 
      @open-templates="handleOpenTemplates"
      @scheduled-publish="handleScheduledPublish"
    />
  </div>
</template>

<script setup lang="ts">
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
  scheduledPublish: []
}>();

function handleSaveTemplate(data: SaveTemplateData) {
  emit('saveTemplate', data);
}

function handleOpenTemplates() {
  emit('openTemplates');
}

function handleScheduledPublish() {
  emit('scheduledPublish');
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

.message-list {
  flex: 1;
  min-height: 200px;
}

.publish-panel {
  flex-shrink: 0;
}
</style>
