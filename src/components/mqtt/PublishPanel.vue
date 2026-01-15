<template>
  <div class="publish-panel app-card">
    <div class="publish-form">
      <!-- 第一行：Topic、QoS、Retain、发布按钮 -->
      <div class="form-row main-row">
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

        <el-select v-model="publishData.qos" style="width: 120px" size="default">
          <el-option :value="0" label="QoS 0" />
          <el-option :value="1" label="QoS 1" />
          <el-option :value="2" label="QoS 2" />
        </el-select>

        <el-checkbox v-model="publishData.retain" label="Retain" />

        <el-button type="primary" :icon="Promotion" @click="handlePublish">
          发布
        </el-button>
      </div>

      <!-- 第二行：Payload输入 -->
      <div class="form-row payload-row">
        <el-input
          v-model="publishData.payload"
          type="textarea"
          :rows="2"
          placeholder='Payload: {"action": "start", "value": 100}'
          resize="none"
          class="payload-input"
        />
        <div class="payload-actions">
          <el-tooltip content="格式化 JSON" placement="top">
            <el-button :icon="MagicStick" size="small" @click="handleFormat" />
          </el-tooltip>
          <el-tooltip content="保存为模板" placement="top">
            <el-button :icon="Star" size="small" @click="handleSaveTemplate" />
          </el-tooltip>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive } from "vue";
import { Promotion, Position, Star, MagicStick } from "@element-plus/icons-vue";
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

.main-row {
  .topic-input {
    flex: 1;
    min-width: 200px;
  }
}

.payload-row {
  align-items: flex-start;
  
  .payload-input {
    flex: 1;
  }

  .payload-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
}
</style>
