<template>
  <div class="message-list app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><ChatDotRound /></el-icon>
        消息列表
        <el-tag size="small" type="info" effect="plain" v-if="messages.length > 0">
          {{ messages.length }}
        </el-tag>
      </span>
      <div class="header-actions">
        <el-segmented
          v-model="formatType"
          :options="formatOptions"
          size="small"
        />
        <el-divider direction="vertical" />
        <el-tooltip content="清空消息" placement="top">
          <el-button text size="small" :icon="Delete" @click="handleClear" />
        </el-tooltip>
        <el-tooltip content="导出消息" placement="top">
          <el-button text size="small" :icon="Download" @click="handleExport" />
        </el-tooltip>
      </div>
    </div>

    <div class="message-container" ref="messageContainer">
      <div
        v-for="msg in messages"
        :key="msg.id"
        class="message-item"
        :class="msg.direction"
      >
        <div class="message-header">
          <span class="msg-direction" :class="msg.direction">
            {{ msg.direction === 'publish' ? 'PUB' : 'RCV' }}
          </span>
          <span class="msg-topic text-ellipsis">{{ msg.topic }}</span>
          <div class="msg-meta">
            <el-tag size="small" effect="plain">Q{{ msg.qos }}</el-tag>
            <el-tag v-if="msg.retain" size="small" type="warning" effect="plain">
              R
            </el-tag>
            <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
          </div>
        </div>
        <div class="message-body">
          <pre class="code-block">{{ formatPayload(msg.payload) }}</pre>
        </div>
      </div>

      <div v-if="messages.length === 0" class="empty-state">
        <el-empty description="暂无消息" :image-size="60">
          <template #description>
            <span>订阅 Topic 后，收到的消息将显示在这里</span>
          </template>
        </el-empty>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ChatDotRound, Delete, Download } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";

type FormatType = "JSON" | "HEX" | "Text";

interface Message {
  id: number;
  direction: "publish" | "receive";
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
  timestamp: Date;
}

const formatOptions = [
  { label: "JSON", value: "JSON" },
  { label: "HEX", value: "HEX" },
  { label: "Text", value: "Text" },
];

const messages = ref<Message[]>([]);
const formatType = ref<FormatType>("JSON");
const messageContainer = ref<HTMLElement>();

const formatTime = (date: Date) => {
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
};

const formatPayload = (payload: string): string => {
  if (formatType.value === "JSON") {
    try {
      return JSON.stringify(JSON.parse(payload), null, 2);
    } catch {
      return payload;
    }
  } else if (formatType.value === "HEX") {
    return Array.from(new TextEncoder().encode(payload))
      .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
      .join(" ");
  }
  return payload;
};

const handleClear = () => {
  messages.value = [];
  ElMessage.success("消息已清空");
};

const handleExport = () => {
  // TODO: 实现消息导出功能
  ElMessage.info("导出功能开发中");
};

// 暴露添加消息的方法给父组件
defineExpose({
  addMessage: (msg: Omit<Message, "id" | "timestamp">) => {
    messages.value.push({
      ...msg,
      id: Date.now(),
      timestamp: new Date(),
    });
    // 滚动到底部
    setTimeout(() => {
      if (messageContainer.value) {
        messageContainer.value.scrollTop = messageContainer.value.scrollHeight;
      }
    }, 0);
  },
});
</script>

<style scoped lang="scss">
.message-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 16px;
  border-bottom: 1px solid var(--app-border-color);
  flex-shrink: 0;
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.message-container {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.message-item {
  padding: 10px 12px;
  border-radius: 6px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);

  &.publish {
    border-left: 3px solid var(--msg-publish);
  }

  &.receive {
    border-left: 3px solid var(--msg-receive);
  }
}

.message-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.msg-topic {
  flex: 1;
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
}

.msg-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.msg-time {
  font-size: 11px;
  color: var(--app-text-secondary);
}

.message-body {
  .code-block {
    max-height: 150px;
    overflow-y: auto;
    font-size: 12px;
    margin: 0;
    padding: 8px 10px;
  }
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
