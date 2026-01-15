<template>
  <div class="main-content">
    <!-- 上部：Server 信息和状态栏 -->
    <ServerInfoBar class="server-info-bar" />

    <!-- 下部：左右分栏 -->
    <div class="content-body">
      <!-- 左侧：订阅列表面板 -->
      <SubscriptionPanel class="subscription-panel" />

      <!-- 右侧：消息区域 -->
      <div class="message-section">
        <!-- 消息列表 -->
        <MessageList ref="messageListRef" class="message-list" />

        <!-- 发布消息 -->
        <PublishPanel class="publish-panel" @publish="handlePublish" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import ServerInfoBar from "./ServerInfoBar.vue";
import SubscriptionPanel from "./SubscriptionPanel.vue";
import MessageList from "./MessageList.vue";
import PublishPanel from "./PublishPanel.vue";

const messageListRef = ref<InstanceType<typeof MessageList>>();

interface PublishData {
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
}

const handlePublish = (data: PublishData) => {
  // 添加到消息列表
  if (messageListRef.value) {
    messageListRef.value.addMessage({
      direction: "publish",
      topic: data.topic,
      payload: data.payload,
      qos: data.qos,
      retain: data.retain,
    });
  }

  // TODO: 调用 Tauri 命令发布消息
  console.log("Publish:", data);
};
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
  gap: 12px;
  min-height: 0;
  overflow: hidden;
}

.subscription-panel {
  width: 280px;
  flex-shrink: 0;
}

.message-section {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
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
