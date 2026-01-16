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
        <el-input
          v-model="searchKeyword"
          placeholder="搜索消息..."
          :prefix-icon="Search"
          size="small"
          style="width: 160px"
          clearable
        />
        <el-dropdown @command="handleFilterCommand">
          <el-button size="small">
            {{ filterLabel }}
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="all">全部</el-dropdown-item>
              <el-dropdown-item command="publish">仅发布</el-dropdown-item>
              <el-dropdown-item command="receive">仅接收</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-divider direction="vertical" />
        <el-tooltip content="清空消息" placement="top">
          <el-button text size="small" :icon="Delete" @click="handleClear" />
        </el-tooltip>
        <!-- <el-tooltip content="导出消息" placement="top">
          <el-button text size="small" :icon="Download" @click="handleExport" />
        </el-tooltip> -->
      </div>
    </div>

    <div class="message-container" ref="messageContainer">
      <div
        v-for="msg in filteredMessages"
        :key="msg.id || msg.timestamp"
        class="message-item"
        :class="[msg.direction, { 'has-error': msg.scriptError }]"
        @click="showDetail(msg)"
      >
        <div class="message-header">
          <span class="msg-direction" :class="[msg.direction, { 'has-error': msg.scriptError }]">
            <el-icon v-if="msg.direction === 'publish'"><Top /></el-icon>
            <el-icon v-else><Bottom /></el-icon>
            {{ msg.direction === "publish" ? "PUB" : "RCV" }}
          </span>
          <span 
            class="msg-topic text-ellipsis" 
            :style="getTopicColor(msg) ? { color: getTopicColor(msg) } : {}"
          >
            <span v-if="getTopicColor(msg)" class="topic-color-dot" :style="{ backgroundColor: getTopicColor(msg) }" />
            {{ msg.topic }}
          </span>
          <div class="msg-meta">
            <el-tag
              v-if="msg.scriptError"
              size="small"
              effect="plain"
              type="danger"
              class="error-tag"
            >
              脚本错误
            </el-tag>
            <el-tag
              size="small"
              effect="plain"
              :type="getFormatTagType(detectPayloadFormat(msg.payload))"
              class="format-tag"
            >
              {{ getFormatLabel(detectPayloadFormat(msg.payload)) }}
            </el-tag>
            <el-tag size="small" effect="plain">Q{{ msg.qos }}</el-tag>
            <el-tag v-if="msg.retain" size="small" type="warning" effect="plain">
              R
            </el-tag>
            <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
          </div>
        </div>
        <div v-if="msg.scriptError" class="message-error">
          <el-icon><WarningFilled /></el-icon>
          <span>{{ msg.scriptError }}</span>
        </div>
        <div class="message-body">
          <MessagePayload :payload="msg.payload" :preview="true" />
        </div>
      </div>

      <div v-if="filteredMessages.length === 0" class="empty-state">
        <el-empty description="暂无消息" :image-size="60">
          <template #description>
            <span v-if="messages.length === 0">
              订阅 Topic 后，收到的消息将显示在这里
            </span>
            <span v-else>没有匹配的消息</span>
          </template>
        </el-empty>
      </div>
    </div>

    <!-- 消息详情对话框 -->
    <el-dialog
      v-model="showDetailDialog"
      :title="selectedMessage?.topic || '消息详情'"
      width="700px"
      class="message-detail-dialog"
    >
      <div v-if="selectedMessage" class="message-detail">
        <el-descriptions :column="3" border size="small">
          <el-descriptions-item label="方向">
            <span class="msg-direction" :class="selectedMessage.direction">
              {{ selectedMessage.direction === "publish" ? "发布" : "接收" }}
            </span>
          </el-descriptions-item>
          <el-descriptions-item label="QoS">
            {{ selectedMessage.qos }}
          </el-descriptions-item>
          <el-descriptions-item label="Retain">
            {{ selectedMessage.retain ? "是" : "否" }}
          </el-descriptions-item>
          <el-descriptions-item label="格式">
            <el-tag
              size="small"
              :type="getFormatTagType(detectPayloadFormat(selectedMessage.payload))"
            >
              {{ getFormatLabel(detectPayloadFormat(selectedMessage.payload)) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="时间" :span="2">
            {{ formatFullTime(selectedMessage.timestamp) }}
          </el-descriptions-item>
          <el-descriptions-item label="主题" :span="3">
            <code class="topic-code">{{ selectedMessage.topic }}</code>
          </el-descriptions-item>
        </el-descriptions>

        <div class="payload-section">
          <div class="payload-header">
            <span class="section-title">消息内容</span>
            <div class="payload-actions">
              <el-button size="small" text :icon="CopyDocument" @click="copyPayload">
                复制内容
              </el-button>
              <el-button
                size="small"
                text
                :icon="Promotion"
                @click="copyToPublish"
              >
                复制到发布
              </el-button>
            </div>
          </div>
          <MessagePayload :payload="selectedMessage.payload" :preview="false" />
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import {
  ChatDotRound,
  Delete,
  Top,
  Bottom,
  ArrowDown,
  Search,
  CopyDocument,
  Promotion,
  WarningFilled,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useServerStore } from "@/stores/server";
import { useMqttStore } from "@/stores/mqtt";
import { useAppStore } from "@/stores/app";
import { useSubscriptionStore } from "@/stores/subscription";
import MessagePayload from "./MessagePayload.vue";
import type { MqttMessage } from "@/types/mqtt";

type PayloadFormat = "json" | "binary" | "text";
type DirectionFilter = "all" | "publish" | "receive";

const serverStore = useServerStore();
const mqttStore = useMqttStore();
const appStore = useAppStore();
const subscriptionStore = useSubscriptionStore();

const messageContainer = ref<HTMLElement>();

// 获取消息的 topic 颜色
function getTopicColor(msg: MqttMessage): string | undefined {
  const serverId = serverStore.activeServerId;
  if (!serverId) return undefined;
  
  // 只有接收的消息才显示订阅颜色
  if (msg.direction !== "receive") return undefined;
  
  const subscription = subscriptionStore.getSubscriptionByTopic(serverId, msg.topic);
  return subscription?.color;
}
const searchKeyword = ref("");
const directionFilter = ref<DirectionFilter>("all");
const showDetailDialog = ref(false);
const selectedMessage = ref<MqttMessage | null>(null);

// 从 MQTT Store 获取消息
const messages = computed(() => {
  const serverId = serverStore.activeServerId;
  if (!serverId) return [];
  return mqttStore.getServerMessages(serverId);
});

// 过滤后的消息
const filteredMessages = computed(() => {
  let result = messages.value;

  // 方向过滤
  if (directionFilter.value !== "all") {
    result = result.filter((m) => m.direction === directionFilter.value);
  }

  // 关键词搜索
  if (searchKeyword.value.trim()) {
    const keyword = searchKeyword.value.toLowerCase();
    result = result.filter((m) => {
      const payloadStr = getPayloadString(m.payload);
      // 同时搜索 HEX 表示（支持二进制消息搜索）
      const hexStr = getPayloadHexString(m.payload);
      return (
        m.topic.toLowerCase().includes(keyword) ||
        payloadStr.toLowerCase().includes(keyword) ||
        hexStr.toLowerCase().includes(keyword)
      );
    });
  }

  return result;
});

// 过滤标签
const filterLabel = computed(() => {
  const labels: Record<DirectionFilter, string> = {
    all: "全部",
    publish: "仅发布",
    receive: "仅接收",
  };
  return labels[directionFilter.value];
});

// 获取 payload 字符串
function getPayloadString(payload: string | Uint8Array | undefined): string {
  if (!payload) return "";
  if (payload instanceof Uint8Array) {
    return new TextDecoder().decode(payload);
  }
  return String(payload);
}

// 获取 payload 的 HEX 字符串（用于二进制消息搜索）
function getPayloadHexString(payload: string | Uint8Array | undefined): string {
  if (!payload) return "";
  const bytes = payload instanceof Uint8Array ? payload : new TextEncoder().encode(payload);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
    .join(" ");
}

// 检测 payload 格式
function detectPayloadFormat(payload: string | Uint8Array | undefined): PayloadFormat {
  if (!payload) return "text";

  const str = getPayloadString(payload);
  const bytes =
    payload instanceof Uint8Array ? payload : new TextEncoder().encode(payload);

  // 尝试检测 JSON
  if (str.trim()) {
    const trimmed = str.trim();
    if (
      (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
      (trimmed.startsWith("[") && trimmed.endsWith("]"))
    ) {
      try {
        JSON.parse(trimmed);
        return "json";
      } catch {
        // 不是有效的 JSON
      }
    }
  }

  // 检测二进制数据
  if (bytes.length > 0) {
    let nonPrintableCount = 0;
    for (const byte of bytes) {
      if ((byte < 32 || byte > 126) && byte !== 9 && byte !== 10 && byte !== 13) {
        nonPrintableCount++;
      }
    }
    if (nonPrintableCount / bytes.length > 0.1) {
      return "binary";
    }
  }

  return "text";
}

// 获取格式标签类型
function getFormatTagType(
  format: PayloadFormat
): "info" | "success" | "warning" {
  const types: Record<PayloadFormat, "info" | "success" | "warning"> = {
    json: "success",
    binary: "warning",
    text: "info",
  };
  return types[format];
}

// 获取格式标签文本
function getFormatLabel(format: PayloadFormat): string {
  const labels: Record<PayloadFormat, string> = {
    json: "JSON",
    binary: "BIN",
    text: "TEXT",
  };
  return labels[format];
}

const formatTime = (timestamp?: string) => {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  const timeStr = date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  // 添加毫秒
  const ms = date.getMilliseconds().toString().padStart(3, "0");
  return `${timeStr}.${ms}`;
};

const formatFullTime = (timestamp?: string) => {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  return date.toLocaleString("zh-CN");
};

function handleFilterCommand(command: string) {
  directionFilter.value = command as DirectionFilter;
}

const handleClear = async () => {
  const serverId = serverStore.activeServerId;
  if (!serverId) return;

  try {
    await ElMessageBox.confirm("确定要清空所有消息记录吗？", "确认清空", {
      type: "warning",
    });
    mqttStore.clearMessages(serverId);
    ElMessage.success("消息已清空");
  } catch {
    // 用户取消
  }
};

function showDetail(message: MqttMessage) {
  selectedMessage.value = message;
  showDetailDialog.value = true;
}

function copyPayload() {
  if (selectedMessage.value) {
    const payload = getPayloadString(selectedMessage.value.payload);
    navigator.clipboard.writeText(payload);
    ElMessage.success("已复制到剪贴板");
  }
}

function copyToPublish() {
  if (selectedMessage.value) {
    const payload = getPayloadString(selectedMessage.value.payload);
    const format = detectPayloadFormat(selectedMessage.value.payload);
    appStore.setCopyToPublish({
      topic: selectedMessage.value.topic,
      payload: payload,
      qos: selectedMessage.value.qos,
      retain: selectedMessage.value.retain,
      payloadType: format,
    });
    ElMessage.success("已复制到发布面板");
    showDetailDialog.value = false;
  }
}
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
  border-radius: 8px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);
    transform: translateX(2px);
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
  gap: 8px;
  margin-bottom: 8px;
}

.msg-direction {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
  flex-shrink: 0;

  .el-icon {
    font-size: 10px;
  }

  &.publish {
    background-color: rgba(59, 130, 246, 0.15);
    color: var(--msg-publish);
  }

  &.receive {
    background-color: rgba(34, 197, 94, 0.15);
    color: var(--msg-receive);
  }
}

.msg-topic {
  flex: 1;
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
}

.topic-color-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.msg-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.format-tag {
  font-size: 10px;
  padding: 0 6px;
  height: 18px;
  line-height: 18px;
}

.msg-time {
  font-size: 11px;
  color: var(--app-text-secondary);
  margin-left: 4px;
}

.message-body {
  margin-top: 6px;
}

.message-error {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  margin-top: 6px;
  margin-bottom: 6px;
  background-color: var(--el-color-danger-light-9);
  border-radius: 4px;
  font-size: 12px;
  color: var(--el-color-danger);
  
  .el-icon {
    flex-shrink: 0;
  }
  
  span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.message-item.has-error {
  border-left-color: var(--el-color-danger);
}

.msg-direction.has-error {
  background-color: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}

.error-tag {
  margin-right: 4px;
}

.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

// 消息详情弹窗样式
.message-detail {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.payload-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.payload-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.section-title {
  font-weight: 600;
  font-size: 14px;
  color: var(--app-text-color);
}

.payload-actions {
  display: flex;
  gap: 8px;
}

.topic-code {
  font-family: "Fira Code", "Consolas", monospace;
  background-color: var(--sidebar-bg);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
}
</style>
