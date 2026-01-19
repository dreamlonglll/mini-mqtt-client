<template>
  <div class="publish-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Promotion /></el-icon>
        {{ $t('publish.send') }}
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
        <el-button 
          size="small" 
          :icon="props.scheduledPublishRunning ? Loading : Timer" 
          :type="props.scheduledPublishRunning ? 'primary' : 'default'"
          :class="{ 'is-running': props.scheduledPublishRunning }"
          @click="handleScheduledPublish"
        >
          {{ props.scheduledPublishRunning ? $t('scheduled.running') : $t('publish.scheduledPublish') }}
        </el-button>
      </div>
    </div>

    <div class="publish-form">
      <!-- 第一行：Topic、QoS、Retain -->
      <div class="form-row">
        <div class="form-item topic-input">
          <el-input
            v-model="publishData.topic"
            :placeholder="$t('publish.topicPlaceholder')"
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
          <el-tooltip :content="$t('publish.openTemplates')" placement="top">
            <el-button :icon="FolderOpened" @click="handleOpenTemplates" />
          </el-tooltip>
          <el-tooltip :content="$t('publish.saveTemplate')" placement="top">
            <el-button :icon="Star" @click="handleSaveTemplate" />
          </el-tooltip>
          <el-button
            type="primary"
            :icon="Promotion"
            :loading="publishing"
            :disabled="!isConnected"
            @click="handlePublish"
          >
            {{ $t('publish.send') }}
          </el-button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Promotion, Position, Star, FolderOpened, Timer, Loading } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invoke } from "@tauri-apps/api/core";
import { useServerStore } from "@/stores/server";
import { useMessageStore } from "@/stores/message";
import { useMqttStore } from "@/stores/mqtt";
import { useAppStore } from "@/stores/app";
import { useEnvStore } from "@/stores/env";
import { ScriptEngine } from "@/utils/scriptEngine";
import type { Script } from "@/stores/script";
import { validatePublishTopic, handleMqttError } from "@/utils/mqttErrorHandler";
import { handleScriptError } from "@/utils/errorHandler";

const { t } = useI18n();

type PayloadFormat = "json" | "hex" | "text";

const props = defineProps<{
  scheduledPublishRunning: boolean;
}>();

const formatOptions = [
  { label: "JSON", value: "json" },
  { label: "HEX", value: "hex" },
  { label: "Text", value: "text" },
];

const serverStore = useServerStore();
const messageStore = useMessageStore();
const mqttStore = useMqttStore();
const appStore = useAppStore();
const envStore = useEnvStore();

const publishing = ref(false);

// 监听复制到发布的消息
watch(
  () => appStore.copyToPublishData,
  (data) => {
    if (data) {
      publishData.topic = data.topic;
      publishData.payload = data.payload;
      publishData.qos = data.qos;
      publishData.retain = data.retain;

      // 设置格式类型
      if (data.payloadType) {
        payloadFormat.value = data.payloadType as PayloadFormat;
      } else if (data.payload.trim()) {
        // 自动检测格式
        try {
          JSON.parse(data.payload.trim());
          payloadFormat.value = "json";
        } catch {
          payloadFormat.value = "text";
        }
      }

      // 清除复制数据
      appStore.clearCopyToPublish();
    }
  }
);

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
  return t('publish.payloadPlaceholder');
});

const emit = defineEmits<{
  saveTemplate: [data: { topic: string; payload: string; qos: number; retain: boolean; payloadType: string }]
  openTemplates: []
  scheduledPublish: []
}>();

// 打开模板管理
const handleOpenTemplates = () => {
  emit("openTemplates");
};

// 打开定时发布
const handleScheduledPublish = () => {
  emit("scheduledPublish");
};

const handleSaveTemplate = () => {
  if (!publishData.topic.trim()) {
    ElMessage.warning(t('errors.inputTopic'));
    return;
  }
  emit("saveTemplate", {
    topic: publishData.topic,
    payload: publishData.payload,
    qos: publishData.qos,
    retain: publishData.retain,
    payloadType: payloadFormat.value,
  });
};

const handlePublish = async () => {
  // 验证 Topic
  const topicValidation = validatePublishTopic(publishData.topic);
  if (!topicValidation.valid) {
    ElMessage.warning(topicValidation.error || t('errors.inputTopic'));
    return;
  }

  const serverId = serverStore.activeServerId;
  if (!serverId) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }

  // 验证格式
  if (payloadFormat.value === "hex") {
    const hex = publishData.payload.replace(/\s/g, "");
    if (!/^[0-9A-Fa-f]*$/.test(hex)) {
      ElMessage.warning(t('errors.hexInvalid'));
      return;
    }
  }

  if (payloadFormat.value === "json" && publishData.payload.trim()) {
    try {
      JSON.parse(publishData.payload);
    } catch {
      ElMessage.warning(t('errors.jsonInvalid'));
      return;
    }
  }

  publishing.value = true;
  try {
    // 确保加载环境变量
    if (envStore.variables.length === 0) {
      await envStore.loadVariables(serverId);
    }
    
    // 替换环境变量
    const processedTopic = envStore.replaceVariables(publishData.topic);
    let processedPayload = envStore.replaceVariables(publishData.payload);
    let scriptError: string | undefined = undefined;
    
    // 应用发送前处理脚本
    try {
      const scripts = await invoke<Script[]>("get_enabled_scripts", {
        serverId,
        scriptType: "before_publish",
      });
      if (scripts.length > 0) {
        processedPayload = await ScriptEngine.executeBeforePublish(
          scripts, 
          processedPayload, 
          processedTopic,
          envStore.variablesMap
        );
      }
    } catch (error: any) {
      // 记录脚本错误
      scriptError = error?.message || String(error);
      // 使用脚本错误处理器（会写入日志）
      handleScriptError(error);
      
      // 将原始消息添加到列表中（带错误标记，不实际发布）
      mqttStore.addPublishMessage(serverId, {
        topic: processedTopic,
        payload: publishData.payload,
        qos: publishData.qos as 0 | 1 | 2,
        retain: publishData.retain,
        scriptError: scriptError,
        payload_type: payloadFormat.value,
      });
      
      ElMessage.error(`${t('script.testError')}: ${scriptError}`);
      return;
    }

    // 调用 messageStore 发布消息（保存到数据库）
    await messageStore.publishMessage(serverId, {
      topic: processedTopic,
      payload: processedPayload,
      qos: publishData.qos,
      retain: publishData.retain,
      format: payloadFormat.value,
    });

    // 同时添加到 mqttStore 的消息列表（用于实时显示）
    mqttStore.addPublishMessage(serverId, {
      topic: processedTopic,
      payload: processedPayload,
      qos: publishData.qos as 0 | 1 | 2,
      retain: publishData.retain,
      payload_type: payloadFormat.value,
    });

    ElMessage.success(t('success.published'));
  } catch (error: any) {
    // 使用 MQTT 错误处理器
    handleMqttError(error?.message || String(error));
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


.is-running {
  :deep(.el-icon) {
    animation: spin 1s linear infinite;
  }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
