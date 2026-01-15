<template>
  <div class="publish-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Promotion /></el-icon>
        发布消息
      </span>
      <div class="header-actions">
        <el-select v-model="payloadFormat" size="small" style="width: 90px">
          <el-option
            v-for="opt in formatOptions"
            :key="opt.value"
            :label="opt.label"
            :value="opt.value"
          />
        </el-select>
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
          <el-tooltip content="保存为模板" placement="top">
            <el-button :icon="Star" @click="handleSaveTemplate" />
          </el-tooltip>
          <el-button
            type="primary"
            :icon="Promotion"
            :loading="publishing"
            :disabled="!isConnected"
            @click="handlePublish"
          >
            发布
          </el-button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, computed } from "vue";
import { Promotion, Position, Star } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useServerStore } from "@/stores/server";
import { useMessageStore } from "@/stores/message";
import { useMqttStore } from "@/stores/mqtt";

type PayloadFormat = "json" | "hex" | "text";

const formatOptions = [
  { label: "JSON", value: "json" },
  { label: "HEX", value: "hex" },
  { label: "Text", value: "text" },
];

const serverStore = useServerStore();
const messageStore = useMessageStore();
const mqttStore = useMqttStore();

const publishing = ref(false);

const publishData = reactive({
  topic: "",
  payload: "",
  qos: 0,
  retain: false,
});

const payloadFormat = ref<PayloadFormat>("json");

const isConnected = computed(() => {
  const serverId = serverStore.activeServerId;
  if (!serverId) return false;
  return mqttStore.getConnectionStatus(serverId) === "connected";
});

const payloadPlaceholder = computed(() => {
  switch (payloadFormat.value) {
    case "json":
      return 'JSON 格式，例如: {"action": "start", "value": 100}';
    case "hex":
      return "十六进制格式，例如: 48 65 6C 6C 6F";
    default:
      return "纯文本消息内容";
  }
});

const handleSaveTemplate = () => {
  // TODO: 保存为命令模板
  ElMessage.success("模板保存功能开发中");
};

const handlePublish = async () => {
  if (!publishData.topic.trim()) {
    ElMessage.warning("请输入 Topic");
    return;
  }

  const serverId = serverStore.activeServerId;
  if (!serverId) {
    ElMessage.warning("请先选择一个服务器");
    return;
  }

  // 验证格式
  if (payloadFormat.value === "hex") {
    const hex = publishData.payload.replace(/\s/g, "");
    if (!/^[0-9A-Fa-f]*$/.test(hex)) {
      ElMessage.warning("HEX 格式不正确");
      return;
    }
  }

  if (payloadFormat.value === "json" && publishData.payload.trim()) {
    try {
      JSON.parse(publishData.payload);
    } catch {
      ElMessage.warning("JSON 格式不正确");
      return;
    }
  }

  publishing.value = true;
  try {
    await messageStore.publishMessage(serverId, {
      topic: publishData.topic,
      payload: publishData.payload,
      qos: publishData.qos,
      retain: publishData.retain,
      format: payloadFormat.value,
    });
    ElMessage.success("消息已发布");
  } catch (error) {
    ElMessage.error(`发布失败: ${error}`);
  } finally {
    publishing.value = false;
  }
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
