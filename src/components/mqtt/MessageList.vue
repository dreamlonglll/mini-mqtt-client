<template>
  <div class="message-list app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><ChatDotRound /></el-icon>
        消息列表
      </span>
      <div class="header-actions">
        <el-select v-model="formatType" size="small" style="width: 100px">
          <el-option label="JSON" value="json" />
          <el-option label="HEX" value="hex" />
          <el-option label="纯文本" value="text" />
        </el-select>
        <el-button text size="small" :icon="Delete" @click="handleClear">
          清空
        </el-button>
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
          <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
        </div>
        <div class="message-body">
          <div class="code-block">{{ formatPayload(msg.payload) }}</div>
        </div>
        <div class="message-meta">
          <el-tag size="small" effect="plain">QoS {{ msg.qos }}</el-tag>
          <el-tag v-if="msg.retain" size="small" type="warning" effect="plain">
            Retain
          </el-tag>
        </div>
      </div>

      <div v-if="messages.length === 0" class="empty-state">
        <el-empty description="暂无消息" :image-size="80">
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
import { ChatDotRound, Delete } from "@element-plus/icons-vue";

interface Message {
  id: number;
  direction: "publish" | "receive";
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
  timestamp: Date;
}

const messages = ref<Message[]>([]);
const formatType = ref<"json" | "hex" | "text">("json");
const messageContainer = ref<HTMLElement>();

const formatTime = (date: Date) => {
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
};

const formatPayload = (payload: string) => {
  if (formatType.value === "json") {
    try {
      return JSON.stringify(JSON.parse(payload), null, 2);
    } catch {
      return payload;
    }
  } else if (formatType.value === "hex") {
    return Array.from(new TextEncoder().encode(payload))
      .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
      .join(" ");
  }
  return payload;
};

const handleClear = () => {
  messages.value = [];
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
  flex: 1;
  min-height: 0;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
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
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.message-item {
  padding: 12px;
  border-radius: 8px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  transition: transform 0.2s ease, box-shadow 0.2s ease;

  &:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

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
  gap: 10px;
  margin-bottom: 8px;
}

.msg-topic {
  flex: 1;
  font-size: 13px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
}

.msg-time {
  font-size: 12px;
  color: var(--app-text-secondary);
}

.message-body {
  margin-bottom: 8px;

  .code-block {
    max-height: 200px;
    overflow-y: auto;
    font-size: 12px;
  }
}

.message-meta {
  display: flex;
  gap: 6px;
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
