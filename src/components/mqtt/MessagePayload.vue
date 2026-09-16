<template>
  <div class="message-payload">
    <!-- JSON 格式 - 保持原样展示，不格式化 -->
    <div v-if="format === 'json'" class="payload-content json-content">
      <pre>{{ textWithLineBreaks }}</pre>
    </div>

    <!-- 二进制/HEX 格式：完整的 HEX + ASCII 展示（按需增量渲染，避免大 payload 一次性生成数万 DOM 节点） -->
    <div v-else-if="format === 'binary'" class="payload-content hex-content">
      <div class="hex-display">
        <div class="hex-row" v-for="(row, index) in hexRows" :key="index">
          <span class="offset">{{ formatOffset(index * 16) }}</span>
          <span class="hex-bytes">
            <span
              v-for="(byte, i) in row.bytes"
              :key="i"
              class="byte"
              :class="{ separator: i === 7 }"
            >{{ byte }}</span>
          </span>
          <span class="ascii">{{ row.ascii }}</span>
        </div>
        <div v-if="hasMoreHex" class="hex-more">
          <span class="hex-more-hint">
            {{ $t('messages.hexPartial', { shown: hexVisibleBytes, total: payloadBytes.length }) }}
          </span>
          <el-button text type="primary" size="small" @click="loadMoreHex">
            {{ $t('messages.hexLoadMore') }}
          </el-button>
        </div>
      </div>
    </div>

    <!-- 纯文本格式 -->
    <div v-else class="payload-content text-content">
      <pre>{{ textWithLineBreaks }}</pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { MqttMessage } from "@/types/mqtt";
import { getDecodedText, getDisplayFormat } from "@/utils/messageDerived";

// 详情模式 HEX 视图每次渲染的字节数（1024 字节 = 64 行）
const HEX_PAGE_BYTES = 1024;

// 只服务于详情弹窗：列表行的预览由 MessageList 用原生元素直接渲染
const props = defineProps<{
  message: MqttMessage;
}>();

// 展示格式与文本全部读消息对象上的 memo
const format = computed(() => getDisplayFormat(props.message));

const payloadBytes = computed(() => props.message.payload ?? new Uint8Array());

// 带换行符标记的文本：在换行符前添加 ↵ 符号标记原始换行位置
const textWithLineBreaks = computed(() =>
  getDecodedText(props.message).replace(/\r?\n/g, "↵$&")
);

// HEX 视图当前已渲染的字节数（切换消息时重置）
const hexVisibleBytes = ref(HEX_PAGE_BYTES);

watch(
  () => payloadBytes.value,
  () => {
    hexVisibleBytes.value = HEX_PAGE_BYTES;
  }
);

// 是否还有未渲染的字节
const hasMoreHex = computed(() => payloadBytes.value.length > hexVisibleBytes.value);

// 再多渲染一页
function loadMoreHex() {
  hexVisibleBytes.value += HEX_PAGE_BYTES;
}

// HEX 行数据（只取当前可见的字节，防止 64KB payload 一次生成数万节点）
const hexRows = computed(() => {
  const bytes = payloadBytes.value.subarray(0, hexVisibleBytes.value);
  const rows: { bytes: string[]; ascii: string }[] = [];

  for (let i = 0; i < bytes.length; i += 16) {
    const rowBytes = Array.from(bytes.slice(i, i + 16));
    const hexBytes = rowBytes.map((b) =>
      b.toString(16).padStart(2, "0").toUpperCase()
    );

    // 填充到 16 字节
    while (hexBytes.length < 16) {
      hexBytes.push("  ");
    }

    const ascii = rowBytes
      .map((b) => (b >= 32 && b <= 126 ? String.fromCharCode(b) : "."))
      .join("");

    rows.push({ bytes: hexBytes, ascii });
  }

  return rows;
});

// 格式化偏移量
function formatOffset(offset: number) {
  return offset.toString(16).toUpperCase().padStart(8, "0");
}
</script>

<style scoped lang="scss">
.message-payload {
  font-family: "Fira Code", "JetBrains Mono", "Consolas", monospace;
  font-size: 12px;
  line-height: 1.5;
}

.payload-content {
  max-height: 400px;
  overflow-y: auto;
  padding: 8px 10px;
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;

  pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-all;
  }
}

.json-content {
  pre {
    color: var(--msg-publish);
  }
}

.hex-display {
  overflow-x: auto;
}

.hex-more {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-top: 6px;
  margin-top: 6px;
  border-top: 1px dashed var(--app-border-color);

  .hex-more-hint {
    color: var(--app-text-secondary);
    font-size: 12px;
  }
}

.hex-row {
  display: flex;
  gap: 12px;
  white-space: nowrap;

  &:hover {
    background-color: var(--sidebar-hover);
  }
}

.offset {
  color: var(--app-text-secondary);
  min-width: 72px;
  user-select: none;
}

.hex-bytes {
  color: var(--msg-publish);
  display: flex;
  gap: 4px;

  .byte {
    min-width: 18px;
    text-align: center;

    &.separator {
      margin-right: 8px;
    }
  }
}

.ascii {
  color: var(--status-connected);
  min-width: 140px;
  padding-left: 12px;
  border-left: 1px solid var(--app-border-color);
}

.text-content {
  pre {
    color: var(--app-text-color);
  }
}
</style>
