<template>
  <div class="message-list app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><ChatDotRound /></el-icon>
        {{ $t('messages.title') }}
        <el-tag size="small" type="info" effect="plain" v-if="messageCount > 0">
          {{ messageCount }}
        </el-tag>
      </span>
      <div class="header-actions">
        <el-input
          v-model="searchKeyword"
          :placeholder="$t('template.searchPlaceholder')"
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
              <el-dropdown-item command="all">{{ $t('template.allCategories') }}</el-dropdown-item>
              <el-dropdown-item command="publish">{{ $t('messages.direction.sent') }}</el-dropdown-item>
              <el-dropdown-item command="receive">{{ $t('messages.direction.received') }}</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-divider direction="vertical" />
        <el-tooltip :content="$t('messages.clear')" placement="top">
          <el-button text size="small" :icon="Delete" @click="handleClear" />
        </el-tooltip>
      </div>
    </div>

    <!--
      固定行高的回收式虚拟滚动：行内容全部裁剪到固定尺寸，
      因此不需要逐行测量、ResizeObserver 与行高缓存，每次 flush 的开销只与可见行数有关。
      行内不使用组件库组件：每行十来个组件实例在高频刷新下是主要的 patch 成本。
    -->
    <RecycleScroller
      :items="filteredMessages"
      :item-size="ROW_HEIGHT"
      key-field="id"
      class="message-container"
    >
      <template #default="{ item: msg }">
        <div class="message-item-wrapper">
          <div
            class="message-item"
            :class="[msg.direction, { 'has-error': msg.scriptError }]"
            @click="showDetail(msg)"
          >
            <div class="message-header">
              <span class="msg-direction" :class="[msg.direction, { 'has-error': msg.scriptError }]">
                <svg class="msg-icon" viewBox="0 0 1024 1024" aria-hidden="true">
                  <path fill="currentColor" :d="msg.direction === 'publish' ? ICON_UP : ICON_DOWN" />
                </svg>
                {{ msg.direction === "publish" ? "PUB" : "RCV" }}
              </span>
              <span class="msg-topic" :style="topicStyle(msg)">
                <span v-if="getTopicColor(msg)" class="topic-color-dot" :style="{ backgroundColor: getTopicColor(msg) }" />
                <span class="topic-text text-ellipsis">{{ msg.topic }}</span>
              </span>
              <div class="msg-meta">
                <span v-if="msg.scriptError" class="tag tag-danger">{{ $t('script.testError') }}</span>
                <span class="tag" :class="FORMAT_TAG_CLASS[getDisplayFormat(msg)]">
                  {{ FORMAT_LABEL[getDisplayFormat(msg)] }}
                </span>
                <span
                  v-if="msg.truncated"
                  class="tag tag-danger"
                  :title="$t('messages.truncatedTip', { size: msg.originalLength })"
                >
                  {{ $t('messages.truncated') }}
                </span>
                <span class="tag">Q{{ msg.qos }}</span>
                <span v-if="msg.retain" class="tag tag-warning">R</span>
                <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
              </div>
            </div>
            <div class="message-body">
              <div v-if="msg.scriptError" class="message-error" :title="msg.scriptError">
                <svg class="msg-icon" viewBox="0 0 1024 1024" aria-hidden="true">
                  <path fill="currentColor" :d="ICON_WARNING" />
                </svg>
                <span class="text-ellipsis">{{ msg.scriptError }}</span>
              </div>
              <pre
                class="message-preview"
                :class="[`is-${getDisplayFormat(msg)}`, { 'with-error': msg.scriptError }]"
              >{{ getPreviewText(msg) }}</pre>
            </div>
          </div>
        </div>
      </template>
      <template #empty>
        <div class="empty-state">
          <el-empty :description="$t('messages.noMessages')" :image-size="60" />
        </div>
      </template>
    </RecycleScroller>

    <!-- 消息详情对话框 -->
    <el-dialog
      v-model="showDetailDialog"
      :title="selectedMessage?.topic || $t('messages.viewPayload')"
      width="700px"
      class="message-detail-dialog"
    >
      <div v-if="selectedMessage" class="message-detail">
        <el-descriptions :column="3" border size="small">
          <el-descriptions-item :label="$t('messages.direction.sent')">
            <span class="msg-direction" :class="selectedMessage.direction">
              {{ selectedMessage.direction === "publish" ? $t('messages.direction.sent') : $t('messages.direction.received') }}
            </span>
          </el-descriptions-item>
          <el-descriptions-item :label="$t('publish.qos')">
            {{ selectedMessage.qos }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('publish.retain')">
            {{ selectedMessage.retain ? "Yes" : "No" }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('publish.payloadType')">
            <el-tag
              size="small"
              :type="FORMAT_TAG_TYPE[getDisplayFormat(selectedMessage)]"
            >
              {{ FORMAT_LABEL[getDisplayFormat(selectedMessage)] }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="Time" :span="2">
            {{ formatFullTime(selectedMessage.timestamp) }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('publish.topic')" :span="3">
            <code class="topic-code">{{ selectedMessage.topic }}</code>
          </el-descriptions-item>
        </el-descriptions>

        <div class="payload-section">
          <div class="payload-header">
            <span class="section-title">{{ $t('publish.payload') }}</span>
            <div class="payload-actions">
              <el-button size="small" text :icon="CopyDocument" @click="copyPayload">
                {{ $t('messages.copyPayload') }}
              </el-button>
              <el-button
                size="small"
                text
                :icon="Promotion"
                @click="copyToPublish"
              >
                {{ $t('messages.copyToPublish') }}
              </el-button>
            </div>
          </div>
          <MessagePayload :message="selectedMessage" />
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  ChatDotRound,
  Delete,
  ArrowDown,
  Search,
  CopyDocument,
  Promotion,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { RecycleScroller } from "vue-virtual-scroller";
import { useServerStore } from "@/stores/server";
import { useMqttStore } from "@/stores/mqtt";
import { useAppStore } from "@/stores/app";
import { useSubscriptionStore } from "@/stores/subscription";
import MessagePayload from "./MessagePayload.vue";
import type { MqttMessage } from "@/types/mqtt";
import { debounce } from "@/utils/debounce";
import { createIncrementalMessageFilter, type DirectionFilter } from "@/utils/messageFilter";
import {
  getDecodedText,
  getHexText,
  getDisplayFormat,
  getPreviewText,
  formatMsgTime,
  formatMsgFullTime,
  type PayloadDisplayFormat,
} from "@/utils/messageDerived";

const { t } = useI18n();

/**
 * 行高（像素）：与下方样式里 wrapper / header / body 的固定尺寸严格对应
 * 8 (wrapper 上下 padding) + 2 (边框) + 16 (item 上下 padding) + 20 (header) + 6 (间距) + 68 (body)
 */
const ROW_HEIGHT = 120;

// 行内图标直接内联 SVG 路径（取自 Element Plus 的 Top / Bottom / WarningFilled），不走组件
const ICON_UP =
  "M572.235 205.282v600.365a30.118 30.118 0 1 1-60.235 0V205.282L292.382 438.633a28.913 28.913 0 0 1-42.646 0 33.43 33.43 0 0 1 0-45.236l271.058-288.045a28.913 28.913 0 0 1 42.647 0L834.5 393.397a33.43 33.43 0 0 1 0 45.176 28.913 28.913 0 0 1-42.647 0l-219.618-233.23z";
const ICON_DOWN =
  "M544 805.888V168a32 32 0 1 0-64 0v637.888L246.656 557.952a30.72 30.72 0 0 0-45.312 0 35.52 35.52 0 0 0 0 48.064l288 306.048a30.72 30.72 0 0 0 45.312 0l288-306.048a35.52 35.52 0 0 0 0-48 30.72 30.72 0 0 0-45.312 0L544 805.824z";
const ICON_WARNING =
  "M512 64a448 448 0 1 1 0 896 448 448 0 0 1 0-896m0 192a58.43 58.43 0 0 0-58.24 63.744l23.36 256.384a35.072 35.072 0 0 0 69.76 0l23.296-256.384A58.43 58.43 0 0 0 512 256m0 512a51.2 51.2 0 1 0 0-102.4 51.2 51.2 0 0 0 0 102.4";

// 格式 → 标签样式 / 文案（binary 统一显示为 HEX）
const FORMAT_TAG_CLASS: Record<PayloadDisplayFormat, string> = {
  json: "tag-success",
  binary: "tag-warning",
  text: "tag-info",
};
const FORMAT_TAG_TYPE: Record<PayloadDisplayFormat, "info" | "success" | "warning"> = {
  json: "success",
  binary: "warning",
  text: "info",
};
const FORMAT_LABEL: Record<PayloadDisplayFormat, string> = {
  json: "JSON",
  binary: "HEX",
  text: "TEXT",
};

const serverStore = useServerStore();
const mqttStore = useMqttStore();
const appStore = useAppStore();
const subscriptionStore = useSubscriptionStore();

// topic → 颜色缓存（订阅数据变化时整表重建；通配符匹配结果懒加入）
const topicColorMap = computed(() => {
  const cache = new Map<string, string | undefined>();
  const serverId = serverStore.activeServerId;
  if (!serverId) return cache;
  // 遍历订阅以建立精确 topic 映射（同时让 computed 跟踪订阅内容变化）
  for (const sub of subscriptionStore.getSubscriptionsByServer(serverId)) {
    if (!cache.has(sub.topic)) {
      cache.set(sub.topic, sub.color);
    }
  }
  return cache;
});

// 获取消息的 topic 颜色
function getTopicColor(msg: MqttMessage): string | undefined {
  // 只有接收的消息才显示订阅颜色
  if (msg.direction !== "receive") return undefined;
  const serverId = serverStore.activeServerId;
  if (!serverId) return undefined;

  const cache = topicColorMap.value;
  if (cache.has(msg.topic)) return cache.get(msg.topic);
  // 通配符订阅懒匹配一次后缓存
  const color = subscriptionStore.getSubscriptionByTopic(serverId, msg.topic)?.color;
  cache.set(msg.topic, color);
  return color;
}

function topicStyle(msg: MqttMessage) {
  const color = getTopicColor(msg);
  return color ? { color } : undefined;
}

const searchKeyword = ref("");
// 防抖后的搜索关键词（避免每个字符都触发全量过滤）
const debouncedKeyword = ref("");
const applyKeyword = debounce((value: string) => {
  debouncedKeyword.value = value;
}, 300);
watch(searchKeyword, (value) => applyKeyword(value));

const directionFilter = ref<DirectionFilter>("all");
const showDetailDialog = ref(false);
const selectedMessage = ref<MqttMessage | null>(null);

// 当前 Server 的消息数组（store 就地更新同一实例，变化靠 messagesVersion 感知）
function currentMessages(): MqttMessage[] {
  const serverId = serverStore.activeServerId;
  return serverId ? mqttStore.getServerMessages(serverId) : [];
}

const messageCount = computed(() => {
  void mqttStore.messagesVersion;
  return currentMessages().length;
});

// 增量过滤：每次 flush 只过滤新增的消息，旧结果复用（见 utils/messageFilter.ts）
const filterMessages = createIncrementalMessageFilter();

const filteredMessages = computed(() => {
  void mqttStore.messagesVersion;
  return filterMessages(currentMessages(), directionFilter.value, debouncedKeyword.value);
});

// 过滤标签
const filterLabel = computed(() => {
  switch (directionFilter.value) {
    case "all":
      return t('template.allCategories');
    case "publish":
      return t('messages.direction.sent');
    case "receive":
      return t('messages.direction.received');
    default:
      return t('template.allCategories');
  }
});

const formatTime = (timestamp?: number) => {
  return formatMsgTime(timestamp, appStore.getDateLocale());
};

const formatFullTime = (timestamp?: number) => {
  return formatMsgFullTime(timestamp, appStore.getDateLocale());
};

function handleFilterCommand(command: string) {
  directionFilter.value = command as DirectionFilter;
}

const handleClear = async () => {
  const serverId = serverStore.activeServerId;
  if (!serverId) return;

  try {
    await ElMessageBox.confirm(t('messages.clearConfirm'), t('messages.clearTitle'), {
      type: "warning",
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    });
    mqttStore.clearMessages(serverId);
    ElMessage.success(t('success.deleted'));
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
    const format = getDisplayFormat(selectedMessage.value);
    // 二进制数据复制为 HEX 格式
    const payload = format === "binary"
      ? getHexText(selectedMessage.value)
      : getDecodedText(selectedMessage.value);
    navigator.clipboard.writeText(payload);
    ElMessage.success(t('success.copied'));
  }
}

function copyToPublish() {
  if (selectedMessage.value) {
    const format = getDisplayFormat(selectedMessage.value);
    // 二进制数据使用 HEX 格式复制到发布面板
    const payload = format === "binary"
      ? getHexText(selectedMessage.value)
      : getDecodedText(selectedMessage.value);
    appStore.setCopyToPublish({
      topic: selectedMessage.value.topic,
      payload: payload,
      qos: selectedMessage.value.qos,
      retain: selectedMessage.value.retain,
      // 二进制数据设置为 hex 类型
      payloadType: format === "binary" ? "hex" : format,
    });
    ElMessage.success(t('messages.copied'));
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
  min-height: 0;
}

// ===== 固定行高布局：以下尺寸与 ROW_HEIGHT 严格对应，改任一处都要同步 =====
.message-item-wrapper {
  height: 120px;
  padding: 4px 8px;
}

.message-item {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 8px 12px;
  border-radius: 8px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  cursor: pointer;
  overflow: hidden;
  // 显式列出过渡属性，避免虚拟滚动复用 view 调整 transform 时意外触发过渡
  transition: background-color 0.2s ease, border-color 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);
  }

  &.publish {
    border-left: 3px solid var(--msg-publish);
  }

  &.receive {
    border-left: 3px solid var(--msg-receive);
  }
}

.message-header {
  height: 20px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  white-space: nowrap;
}

.msg-icon {
  width: 10px;
  height: 10px;
  flex-shrink: 0;
}

.msg-direction {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  font-size: 11px;
  font-weight: 600;
  padding: 0 8px;
  border-radius: 4px;
  flex-shrink: 0;

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
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);

  .topic-text {
    min-width: 0;
  }
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

// 轻量标签：样式对齐 el-tag 的 plain / small，但不是组件
.tag {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 6px;
  font-size: 10px;
  line-height: 1;
  border-radius: 4px;
  white-space: nowrap;
  color: var(--el-color-primary);
  background-color: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary-light-5);

  &.tag-info {
    color: var(--el-color-info);
    background-color: var(--el-color-info-light-9);
    border-color: var(--el-color-info-light-5);
  }

  &.tag-success {
    color: var(--el-color-success);
    background-color: var(--el-color-success-light-9);
    border-color: var(--el-color-success-light-5);
  }

  &.tag-warning {
    color: var(--el-color-warning);
    background-color: var(--el-color-warning-light-9);
    border-color: var(--el-color-warning-light-5);
  }

  &.tag-danger {
    color: var(--el-color-danger);
    background-color: var(--el-color-danger-light-9);
    border-color: var(--el-color-danger-light-5);
  }
}

.msg-time {
  font-size: 11px;
  color: var(--app-text-secondary);
  margin-left: 4px;
}

.message-body {
  height: 68px;
  padding: 6px 10px;
  overflow: hidden;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  font-family: "Fira Code", "JetBrains Mono", "Consolas", monospace;
  font-size: 12px;
  line-height: 18px;
}

.message-error {
  height: 18px;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--el-color-danger);

  span {
    min-width: 0;
  }
}

.message-preview {
  margin: 0;
  overflow: hidden;
  white-space: pre-wrap;
  word-break: break-all;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  color: var(--app-text-color);

  &.is-json,
  &.is-binary {
    color: var(--msg-publish);
  }

  &.with-error {
    -webkit-line-clamp: 2;
  }
}

.message-item.has-error {
  border-left-color: var(--el-color-danger);
}

.msg-direction.has-error {
  background-color: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
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
