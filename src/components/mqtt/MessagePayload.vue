<template>
  <div class="message-payload" :class="{ preview, expanded: !preview }">
    <!-- JSON 格式 - 保持原样展示，不格式化 -->
    <div v-if="effectiveFormat === 'json'" class="payload-content json-content">
      <pre>{{ preview ? displayPayload : displayPayloadWithLineBreaks }}</pre>
    </div>

    <!-- 二进制/HEX 格式 -->
    <div v-else-if="effectiveFormat === 'binary'" class="payload-content hex-content">
      <!-- 预览模式：只显示简单的 HEX 字符串 -->
      <div v-if="preview" class="hex-preview-simple">
        <span>{{ simpleHexPreview }}</span>
      </div>
      <!-- 详情模式：显示完整的 HEX + ASCII 展示 -->
      <div v-else class="hex-display">
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
      </div>
    </div>

    <!-- 纯文本格式 -->
    <div v-else class="payload-content text-content">
      <pre>{{ preview ? displayPayload : displayPayloadWithLineBreaks }}</pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

type PayloadFormat = "json" | "binary" | "text";

const props = defineProps<{
  payload: string | Uint8Array | undefined;
  preview?: boolean;
  payloadType?: "json" | "hex" | "text";
}>();

// 将 payload 转换为字符串
const payloadString = computed(() => {
  if (!props.payload) return "";
  if (props.payload instanceof Uint8Array) {
    return new TextDecoder().decode(props.payload);
  }
  return String(props.payload);
});

// 将 payload 转换为字节数组
const payloadBytes = computed(() => {
  if (!props.payload) return new Uint8Array();
  if (props.payload instanceof Uint8Array) {
    return props.payload;
  }
  return new TextEncoder().encode(props.payload);
});

// 自动检测格式
const detectedFormat = computed<PayloadFormat>(() => {
  if (!props.payload) return "text";

  const str = payloadString.value;

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

  // 检测二进制数据（包含不可打印字符）
  const bytes = payloadBytes.value;
  if (bytes.length > 0) {
    let nonPrintableCount = 0;
    for (const byte of bytes) {
      // 检查是否为不可打印字符（排除常见的空白字符）
      if ((byte < 32 || byte > 126) && byte !== 9 && byte !== 10 && byte !== 13) {
        nonPrintableCount++;
      }
    }
    // 如果超过 10% 的字符是不可打印的，则认为是二进制
    if (nonPrintableCount / bytes.length > 0.1) {
      return "binary";
    }
  }

  return "text";
});

// 有效格式（优先使用指定的 payloadType，否则使用自动检测）
const effectiveFormat = computed<PayloadFormat>(() => {
  if (props.payloadType) {
    // hex 类型映射为 binary 显示
    if (props.payloadType === "hex") return "binary";
    if (props.payloadType === "json") return "json";
    return "text";
  }
  return detectedFormat.value;
});

// 简单的 HEX 预览（用于列表预览，不包含 offset 和 ASCII）
const simpleHexPreview = computed(() => {
  const bytes = payloadBytes.value;
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0").toUpperCase())
    .join(" ");
});

// 显示的 payload（用于预览或文本显示）
// 始终显示完整内容，不做截断
const displayPayload = computed(() => {
  return payloadString.value;
});

// 带换行符标记的 payload（用于详情展示）
const displayPayloadWithLineBreaks = computed(() => {
  const str = payloadString.value;
  // 在换行符前添加 ↵ 符号标记原始换行位置
  return str.replace(/\r?\n/g, '↵$&');
});

// HEX 行数据
const hexRows = computed(() => {
  const bytes = payloadBytes.value;
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

// 暴露格式类型供外部使用
defineExpose({
  detectedFormat,
  effectiveFormat,
});
</script>

<style scoped lang="scss">
.message-payload {
  font-family: "Fira Code", "JetBrains Mono", "Consolas", monospace;
  font-size: 12px;
  line-height: 1.5;
}

.message-payload.preview {
  .payload-content {
    // 移除高度限制，始终显示完整内容
    max-height: none;
    overflow: visible;
  }
}

.message-payload.expanded {
  .payload-content {
    max-height: 400px;
    overflow-y: auto;
  }
}

.payload-content {
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

.hex-content {
  .hex-preview {
    color: var(--app-text-secondary);
    font-size: 12px;
  }
  
  .hex-preview-simple {
    color: var(--msg-publish);
    font-size: 12px;
    word-break: break-all;
  }
}

.hex-display {
  overflow-x: auto;
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

.raw-text {
  color: var(--app-text-secondary);
}
</style>
