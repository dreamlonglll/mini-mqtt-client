<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? '编辑 Server' : '新增 Server'"
    width="600px"
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="$emit('update:visible', $event)"
  >
    <el-form
      ref="formRef"
      :model="formData"
      :rules="rules"
      label-width="110px"
      label-position="right"
    >
      <el-tabs v-model="activeTab">
        <!-- 基本配置 -->
        <el-tab-pane label="基本配置" name="basic">
          <el-form-item label="名称" prop="name">
            <el-input v-model="formData.name" placeholder="给这个连接起个名字" />
          </el-form-item>

          <!-- 服务器地址：协议 + 主机 + 端口 -->
          <el-form-item label="服务器地址" required>
            <div class="address-input">
              <el-select v-model="formData.protocol" class="protocol-select">
                <el-option label="mqtt://" value="mqtt" />
                <el-option label="mqtts://" value="mqtts" />
                <el-option label="ws://" value="ws" />
                <el-option label="wss://" value="wss" />
              </el-select>
              <el-form-item prop="host" class="host-input" style="margin-bottom: 0">
                <el-input v-model="formData.host" placeholder="主机地址" />
              </el-form-item>
              <span class="separator">:</span>
              <el-form-item prop="port" class="port-input" style="margin-bottom: 0">
                <el-input-number
                  v-model="formData.port"
                  :min="1"
                  :max="65535"
                  :controls="false"
                  placeholder="端口"
                />
              </el-form-item>
            </div>
          </el-form-item>

          <el-form-item label="协议版本" prop="protocol_version">
            <el-radio-group v-model="formData.protocol_version">
              <el-radio value="3.1.1">MQTT 3.1.1</el-radio>
              <el-radio value="5.0">MQTT 5.0</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item label="Client ID" prop="client_id">
            <el-input v-model="formData.client_id" placeholder="留空则自动生成">
              <template #append>
                <el-button @click="generateClientId">
                  <el-icon><RefreshRight /></el-icon>
                </el-button>
              </template>
            </el-input>
          </el-form-item>
        </el-tab-pane>

        <!-- 认证配置 -->
        <el-tab-pane label="认证" name="auth">
          <el-form-item label="用户名" prop="username">
            <el-input v-model="formData.username" placeholder="可选" />
          </el-form-item>

          <el-form-item label="密码" prop="password">
            <el-input
              v-model="formData.password"
              type="password"
              placeholder="可选"
              show-password
            />
          </el-form-item>
        </el-tab-pane>

        <!-- 高级配置 -->
        <el-tab-pane label="高级" name="advanced">
          <el-form-item label="Keep Alive" prop="keep_alive">
            <el-input-number v-model="formData.keep_alive" :min="0" :max="65535" />
          </el-form-item>
          <el-form-item label="Clean Session">
            <el-switch v-model="formData.clean_session" />
          </el-form-item>

          <el-form-item label="TLS/SSL" v-if="formData.protocol === 'mqtts' || formData.protocol === 'wss'">
            <el-switch v-model="formData.use_tls" disabled />
            <span class="form-hint">已根据协议自动启用</span>
          </el-form-item>

          <template v-if="formData.protocol === 'mqtts' || formData.protocol === 'wss'">
            <el-form-item label="CA 证书" prop="ca_cert">
              <el-input
                v-model="formData.ca_cert"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式证书内容（可选）"
              />
            </el-form-item>

            <el-form-item label="客户端证书" prop="client_cert">
              <el-input
                v-model="formData.client_cert"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式证书内容（可选）"
              />
            </el-form-item>

            <el-form-item label="客户端私钥" prop="client_key">
              <el-input
                v-model="formData.client_key"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式私钥内容（可选）"
              />
            </el-form-item>
          </template>
        </el-tab-pane>
      </el-tabs>
    </el-form>

    <template #footer>
      <el-button @click="$emit('update:visible', false)">取消</el-button>
      <el-button type="primary" :loading="saving" @click="handleSave">
        保存
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, watch, computed, reactive } from "vue";
import type { FormInstance, FormRules } from "element-plus";
import { RefreshRight } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useServerStore } from "@/stores/server";
import type { MqttServer } from "@/types/mqtt";

const props = defineProps<{
  visible: boolean;
  server?: MqttServer | null;
}>();

const emit = defineEmits<{
  "update:visible": [value: boolean];
  saved: [];
}>();

const serverStore = useServerStore();
const formRef = ref<FormInstance>();
const activeTab = ref("basic");
const saving = ref(false);

const isEdit = computed(() => !!props.server?.id);

// 表单数据
interface FormData {
  id?: number;
  name: string;
  protocol: "mqtt" | "mqtts" | "ws" | "wss";
  host: string;
  port: number;
  protocol_version: "3.1.1" | "5.0";
  username?: string;
  password?: string;
  client_id?: string;
  keep_alive: number;
  clean_session: boolean;
  use_tls: boolean;
  ca_cert?: string;
  client_cert?: string;
  client_key?: string;
}

const formData = reactive<FormData>({
  name: "",
  protocol: "mqtt",
  host: "",
  port: 1883,
  protocol_version: "5.0",
  username: "",
  password: "",
  client_id: "",
  keep_alive: 60,
  clean_session: true,
  use_tls: false,
  ca_cert: "",
  client_cert: "",
  client_key: "",
});

// 协议变化时自动更新端口和TLS
watch(() => formData.protocol, (newProtocol) => {
  switch (newProtocol) {
    case "mqtt":
      formData.port = 1883;
      formData.use_tls = false;
      break;
    case "mqtts":
      formData.port = 8883;
      formData.use_tls = true;
      break;
    case "ws":
      formData.port = 8083;
      formData.use_tls = false;
      break;
    case "wss":
      formData.port = 8084;
      formData.use_tls = true;
      break;
  }
});

const rules: FormRules = {
  name: [
    { required: true, message: "请输入名称", trigger: "blur" },
    { min: 1, max: 50, message: "名称长度在 1-50 个字符", trigger: "blur" },
  ],
  host: [
    { required: true, message: "请输入主机地址", trigger: "blur" },
  ],
  port: [
    { required: true, message: "请输入端口", trigger: "blur" },
  ],
};

watch(
  () => props.visible,
  (val) => {
    if (val) {
      activeTab.value = "basic";
      if (props.server) {
        // 编辑模式
        formData.id = props.server.id;
        formData.name = props.server.name;
        formData.host = props.server.host;
        formData.port = props.server.port;
        formData.protocol_version = props.server.protocol_version;
        formData.username = props.server.username || "";
        formData.password = props.server.password || "";
        formData.client_id = props.server.client_id || "";
        formData.keep_alive = props.server.keep_alive;
        formData.clean_session = props.server.clean_session;
        formData.use_tls = props.server.use_tls;
        formData.ca_cert = props.server.ca_cert || "";
        formData.client_cert = props.server.client_cert || "";
        formData.client_key = props.server.client_key || "";
        // 根据 use_tls 推断协议
        formData.protocol = props.server.use_tls ? "mqtts" : "mqtt";
      } else {
        // 新增模式：重置表单
        formData.id = undefined;
        formData.name = "";
        formData.protocol = "mqtt";
        formData.host = "";
        formData.port = 1883;
        formData.protocol_version = "5.0";
        formData.username = "";
        formData.password = "";
        formData.client_id = "";
        formData.keep_alive = 60;
        formData.clean_session = true;
        formData.use_tls = false;
        formData.ca_cert = "";
        formData.client_cert = "";
        formData.client_key = "";
      }
    }
  }
);

const generateClientId = () => {
  const random = Math.random().toString(36).substring(2, 10);
  formData.client_id = `mqtt_${Date.now()}_${random}`;
};

const handleSave = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;

  const serverData: MqttServer = {
    id: formData.id,
    name: formData.name,
    host: formData.host,
    port: formData.port,
    protocol_version: formData.protocol_version,
    username: formData.username || undefined,
    password: formData.password || undefined,
    client_id: formData.client_id || undefined,
    keep_alive: formData.keep_alive,
    clean_session: formData.clean_session,
    use_tls: formData.use_tls,
    ca_cert: formData.ca_cert || undefined,
    client_cert: formData.client_cert || undefined,
    client_key: formData.client_key || undefined,
  };

  saving.value = true;
  try {
    if (isEdit.value) {
      await serverStore.updateServer(serverData);
      ElMessage.success("更新成功");
    } else {
      await serverStore.createServer(serverData);
      ElMessage.success("创建成功");
    }
    emit("saved");
    emit("update:visible", false);
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`);
  } finally {
    saving.value = false;
  }
};
</script>

<style scoped lang="scss">
:deep(.el-tabs__content) {
  padding-top: 16px;
}

.address-input {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.protocol-select {
  width: 120px;
  flex-shrink: 0;
}

.host-input {
  flex: 1;
}

.separator {
  color: var(--app-text-secondary);
  font-weight: 500;
}

.port-input {
  width: 100px;
  flex-shrink: 0;
}

.form-hint {
  margin-left: 8px;
  font-size: 12px;
  color: var(--app-text-secondary);
}
</style>
