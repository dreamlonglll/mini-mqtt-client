<template>
  <div class="message-list app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><ChatDotRound /></el-icon>
        {{ $t('messages.title') }}
        <el-tag size="small" type="info" effect="plain" v-if="messages.length > 0">
          {{ messages.length }}
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
        <!-- <el-tooltip content="导出消息" placement="top">
          <el-button text size="small" :icon="Download" @click="handleExport" />
        </el-tooltip> -->
      </div>
    </div>

    <DynamicScroller
      ref="scrollerRef"
      :items="filteredMessages"
      :min-item-size="70"
      class="message-container"
      key-field="id"
    >
      <template #empty>
        <div class="empty-state">
          <el-empty :description="$t('messages.noMessages')" :image-size="60" />
        </div>
      </template>
      <template #default="{ item: msg, index, active }">
        <DynamicScrollerItem
          :item="msg"
          :active="active"
          :data-index="index"
        >
          <div class="message-item-wrapper">
            <div
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
                  {{ $t('script.testError') }}
                </el-tag>
                <el-tag
                  size="small"
                  effect="plain"
                  :type="getFormatTagType(getDisplayFormat(msg))"
                  class="format-tag"
                >
                  {{ getFormatLabel(getDisplayFormat(msg)) }}
                </el-tag>
                <el-tooltip
                  v-if="msg.truncated"
                  :content="$t('messages.truncatedTip', { size: msg.originalLength })"
                  placement="top"
                >
                  <el-tag size="small" type="danger" effect="plain">
                    {{ $t('messages.truncated') }}
                  </el-tag>
                </el-tooltip>
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
              <MessagePayload :payload="msg.payload" :message="msg" :preview="true" :payload-type="msg.payload_type" />
            </div>
            </div>
          </div>
        </DynamicScrollerItem>
      </template>
    </DynamicScroller>

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
              :type="getFormatTagType(getDisplayFormat(selectedMessage))"
            >
              {{ getFormatLabel(getDisplayFormat(selectedMessage)) }}
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
          <MessagePayload :payload="selectedMessage.payload" :message="selectedMessage" :preview="false" :payload-type="selectedMessage.payload_type" />
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
  Top,
  Bottom,
  ArrowDown,
  Search,
  CopyDocument,
  Promotion,
  WarningFilled,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { DynamicScroller, DynamicScrollerItem } from "vue-virtual-scroller";
import { useServerStore } from "@/stores/server";
import { useMqttStore } from "@/stores/mqtt";
import { useAppStore } from "@/stores/app";
import { useSubscriptionStore } from "@/stores/subscription";
import MessagePayload from "./MessagePayload.vue";
import type { MqttMessage } from "@/types/mqtt";
import { debounce } from "@/utils/debounce";
import {
  getDecodedText,
  getHexText,
  getDisplayFormat,
  formatMsgTime,
  formatMsgFullTime,
  type PayloadDisplayFormat,
} from "@/utils/messageDerived";

const { t } = useI18n();

type DirectionFilter = "all" | "publish" | "receive";

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

  // 关键词搜索（读取消息上的派生缓存，不再即算即弃）
  if (debouncedKeyword.value.trim()) {
    const keyword = debouncedKeyword.value.toLowerCase();
    result = result.filter((m) => {
      return (
        m.topic.toLowerCase().includes(keyword) ||
        getDecodedText(m).toLowerCase().includes(keyword) ||
        getHexText(m).toLowerCase().includes(keyword)
      );
    });
  }

  // store 侧 flush 采用就地修改，这里始终返回新数组标识，
  // 让虚拟滚动能感知 items 变化
  return result === messages.value ? result.slice() : result;
});

// ===== 虚拟滚动行高缓存定期重置（防止被裁剪消息的 sizes 记录无限累积） =====
const scrollerRef = ref<{ forceUpdate: (clearCache?: boolean) => void } | null>(null);
let trimmedBaseline = 0;
watch(
  () => mqttStore.trimmedCount,
  (count) => {
    // 每裁剪约 2000 条重置一次行高缓存，可见行会自动重新测量
    if (count - trimmedBaseline >= 2000) {
      trimmedBaseline = count;
      scrollerRef.value?.forceUpdate(true);
    }
  }
);

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

// 获取格式标签类型
function getFormatTagType(
  format: PayloadDisplayFormat
): "info" | "success" | "warning" {
  const types: Record<PayloadDisplayFormat, "info" | "success" | "warning"> = {
    json: "success",
    binary: "warning",
    text: "info",
  };
  return types[format];
}

// 获取格式标签文本
function getFormatLabel(format: PayloadDisplayFormat): string {
  // binary 格式统一显示为 HEX
  const labels: Record<PayloadDisplayFormat, string> = {
    json: "JSON",
    binary: "HEX",
    text: "TEXT",
  };
  return labels[format];
}

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

.empty-container {
  display: flex;
  align-items: center;
  justify-content: center;
}

.message-item-wrapper {
  padding: 4px 8px;
}

.message-item {
  padding: 10px 12px;
  border-radius: 8px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  cursor: pointer;
  // 显式列出过渡属性，避免虚拟滚动复用 view 调整 transform 时意外触发过渡
  transition: background-color 0.2s ease, border-color 0.2s ease;

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
