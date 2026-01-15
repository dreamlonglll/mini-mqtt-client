<template>
  <div class="publish-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Promotion /></el-icon>
        发布消息
      </span>
      <div class="header-actions">
        <el-segmented
          v-model="payloadFormat"
          :options="formatOptions"
          size="small"
        />
      </div>
    </div>

    <div class="publish-form">
      <!-- 第一行：Topic、QoS、Retain -->
      <div class="form-row">
        <div class="form-item topic-input">
          <el-input
            v-model="publishData.topic"
            placeholder="Topic，例如: device/001/command"
            size="default"
          >
            <template #prefix>
              <el-icon><Position /></el-icon>
            </template>
          </el-input>
        </div>

        <el-select v-model="publishData.qos" style="width: 100px" size="default">
          <el-option :value="0" label="QoS 0" />
          <el-option :value="1" label="QoS 1" />
          <el-option :value="2" label="QoS 2" />
        </el-select>

        <el-checkbox v-model="publishData.retain">Retain</el-checkbox>
      </div>

      <!-- 第二行：Payload 输入 -->
      <div class="form-row payload-row">
        <div class="payload-input-wrapper">
          <el-input
            v-model="publishData.payload"
            type="textarea"
            :rows="3"
            :placeholder="payloadPlaceholder"
            resize="none"
            class="payload-input"
          />
        </div>
        <div class="form-actions">
          <el-tooltip :content="formatButtonText" placement="top">
            <el-button :icon="MagicStick" @click="handleFormat" />
          </el-tooltip>
          <el-tooltip content="保存为模板" placement="top">
            <el-button :icon="Star" @click="handleSaveTemplate" />
          </el-tooltip>
          <el-button type="primary" :icon="Promotion" @click="handlePublish">
            发布
          </el-button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, computed } from "vue";
import { Promotion, Position, Star, MagicStick } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";

type PayloadFormat = "JSON" | "HEX" | "Text";

const formatOptions = [
  { label: "JSON", value: "JSON" },
  { label: "HEX", value: "HEX" },
  { label: "Text", value: "Text" },
];

const publishData = reactive({
  topic: "",
  payload: "",
  qos: 0,
  retain: false,
});

const payloadFormat = ref<PayloadFormat>("JSON");

const payloadPlaceholder = computed(() => {
  switch (payloadFormat.value) {
    case "JSON":
      return 'JSON 格式，例如: {"action": "start", "value": 100}';
    case "HEX":
      return "十六进制格式，例如: 48 65 6C 6C 6F";
    default:
      return "纯文本消息内容";
  }
});

const formatButtonText = computed(() => {
  switch (payloadFormat.value) {
    case "JSON":
      return "格式化 JSON";
    case "HEX":
      return "格式化 HEX";
    default:
      return "格式化";
  }
});

const emit = defineEmits<{
  (e: "publish", data: typeof publishData): void;
}>();

const handleFormat = () => {
  if (payloadFormat.value === "JSON") {
    try {
      const parsed = JSON.parse(publishData.payload);
      publishData.payload = JSON.stringify(parsed, null, 2);
      ElMessage.success("JSON 格式化成功");
    } catch {
      ElMessage.warning("Payload 不是有效的 JSON 格式");
    }
  } else if (payloadFormat.value === "HEX") {
    // 格式化 HEX：移除多余空格，每两个字符加空格
    const hex = publishData.payload.replace(/\s/g, "").toUpperCase();
    if (/^[0-9A-F]*$/.test(hex)) {
      publishData.payload = hex.match(/.{1,2}/g)?.join(" ") || "";
      ElMessage.success("HEX 格式化成功");
    } else {
      ElMessage.warning("Payload 包含非法的十六进制字符");
    }
  }
};

const handleSaveTemplate = () => {
  // TODO: 保存为命令模板
  ElMessage.success("模板保存功能开发中");
};

const handlePublish = () => {
  if (!publishData.topic.trim()) {
    ElMessage.warning("请输入 Topic");
    return;
  }

  let payload = publishData.payload;

  // 如果是 HEX 格式，转换为字符串
  if (payloadFormat.value === "HEX") {
    const hex = publishData.payload.replace(/\s/g, "");
    if (!/^[0-9A-Fa-f]*$/.test(hex) || hex.length % 2 !== 0) {
      ElMessage.warning("HEX 格式不正确");
      return;
    }
    payload = hex
      .match(/.{2}/g)
      ?.map((byte) => String.fromCharCode(parseInt(byte, 16)))
      .join("") || "";
  }

  emit("publish", { ...publishData, payload });
  ElMessage.success("消息已发布");
};
</script>

<style scoped lang="scss">
.publish-panel {
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 16px;
  border-bottom: 1px solid var(--app-border-color);
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

.publish-form {
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-row {
  display: flex;
  gap: 10px;
  align-items: center;
}

.topic-input {
  flex: 1;
  min-width: 200px;
}

.payload-row {
  align-items: flex-end;

  .payload-input-wrapper {
    flex: 1;
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }
}
</style>
