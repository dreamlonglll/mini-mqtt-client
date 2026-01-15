<template>
  <div class="main-content">
    <!-- 订阅区域 - 紧凑的单行 -->
    <SubscriptionPanel class="subscription-area" />

    <!-- 消息列表区域 - 占据大部分空间 -->
    <MessageList ref="messageListRef" class="message-area" />

    <!-- 发布消息区域 - 紧凑布局 -->
    <PublishPanel class="publish-area" @publish="handlePublish" />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
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

.subscription-area {
  flex-shrink: 0;
}

.message-area {
  flex: 1;
  min-height: 200px;
}

.publish-area {
  flex-shrink: 0;
}
</style>
