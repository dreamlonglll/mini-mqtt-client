<template>
  <div class="publish-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Promotion /></el-icon>
        发布消息
      </span>
    </div>

    <div class="publish-form">
      <div class="form-row">
        <div class="form-item topic-input">
          <label class="form-label">Topic</label>
          <el-input
            v-model="publishData.topic"
            placeholder="输入 Topic，例如: device/001/command"
          >
            <template #prefix>
              <el-icon><Position /></el-icon>
            </template>
          </el-input>
        </div>

        <div class="form-item qos-select">
          <label class="form-label">QoS</label>
          <el-select v-model="publishData.qos" style="width: 100%">
            <el-option :value="0" label="QoS 0 - 最多一次" />
            <el-option :value="1" label="QoS 1 - 至少一次" />
            <el-option :value="2" label="QoS 2 - 仅一次" />
          </el-select>
        </div>

        <div class="form-item retain-switch">
          <label class="form-label">Retain</label>
          <el-switch v-model="publishData.retain" />
        </div>
      </div>

      <div class="form-row">
        <div class="form-item payload-input">
          <div class="payload-header">
            <label class="form-label">Payload</label>
            <div class="payload-actions">
              <el-button text size="small" @click="handleFormat">
                格式化 JSON
              </el-button>
              <el-button text size="small" @click="handleClear">
                清空
              </el-button>
            </div>
          </div>
          <el-input
            v-model="publishData.payload"
            type="textarea"
            :rows="4"
            placeholder='输入消息内容，例如: {"action": "start", "value": 100}'
            resize="none"
          />
        </div>
      </div>

      <div class="form-actions">
        <el-button @click="handleSaveTemplate" :icon="Star">
          保存为模板
        </el-button>
        <el-button type="primary" :icon="Promotion" @click="handlePublish">
          发布消息
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive } from "vue";
import { Promotion, Position, Star } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";

const publishData = reactive({
  topic: "",
  payload: "",
  qos: 0,
  retain: false,
});

const emit = defineEmits<{
  (e: "publish", data: typeof publishData): void;
}>();

const handleFormat = () => {
  try {
    const parsed = JSON.parse(publishData.payload);
    publishData.payload = JSON.stringify(parsed, null, 2);
  } catch {
    ElMessage.warning("Payload 不是有效的 JSON 格式");
  }
};

const handleClear = () => {
  publishData.payload = "";
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
  emit("publish", { ...publishData });
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
  align-items: center;
  padding: 12px 16px;
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

.publish-form {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form-row {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.form-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--app-text-secondary);
}

.topic-input {
  flex: 1;
}

.qos-select {
  width: 180px;
}

.retain-switch {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding-top: 4px;
}

.payload-input {
  flex: 1;
}

.payload-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.payload-actions {
  display: flex;
  gap: 4px;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--app-border-color);
}
</style>
