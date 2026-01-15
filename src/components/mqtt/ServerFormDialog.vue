<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? '编辑 Server' : '新增 Server'"
    width="580px"
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="$emit('update:visible', $event)"
  >
    <el-form
      ref="formRef"
      :model="formData"
      :rules="rules"
      label-width="100px"
      label-position="right"
    >
      <el-tabs v-model="activeTab">
        <!-- 基本配置 -->
        <el-tab-pane label="基本配置" name="basic">
          <el-form-item label="名称" prop="name">
            <el-input v-model="formData.name" placeholder="给这个连接起个名字" />
          </el-form-item>

          <el-form-item label="服务器地址" prop="address">
            <el-input
              v-model="formData.address"
              placeholder="例如: broker.emqx.io:1883"
            />
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
          <el-row :gutter="16">
            <el-col :span="12">
              <el-form-item label="Keep Alive" prop="keep_alive">
                <el-input-number v-model="formData.keep_alive" :min="0" :max="65535" style="width: 100%" />
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="Clean Session">
                <el-switch v-model="formData.clean_session" />
              </el-form-item>
            </el-col>
          </el-row>

          <el-form-item label="启用 TLS">
            <el-switch v-model="formData.use_tls" />
          </el-form-item>

          <template v-if="formData.use_tls">
            <el-form-item label="CA 证书" prop="ca_cert">
              <el-input
                v-model="formData.ca_cert"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式证书内容"
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

// 表单数据，使用 address 字段代替 host + port
interface FormData {
  id?: number;
  name: string;
  address: string; // host:port 格式
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
  address: "",
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

// 解析地址
const parseAddress = (address: string): { host: string; port: number } => {
  const parts = address.split(":");
  if (parts.length === 2) {
    const port = parseInt(parts[1], 10);
    if (!isNaN(port) && port > 0 && port <= 65535) {
      return { host: parts[0], port };
    }
  }
  // 默认端口 1883
  return { host: address, port: 1883 };
};

// 验证地址格式
const validateAddress = (_rule: unknown, value: string, callback: (error?: Error) => void) => {
  if (!value) {
    callback(new Error("请输入服务器地址"));
    return;
  }
  const { host, port } = parseAddress(value);
  if (!host) {
    callback(new Error("主机地址不能为空"));
    return;
  }
  if (port < 1 || port > 65535) {
    callback(new Error("端口范围应为 1-65535"));
    return;
  }
  callback();
};

const rules: FormRules = {
  name: [
    { required: true, message: "请输入名称", trigger: "blur" },
    { min: 1, max: 50, message: "名称长度在 1-50 个字符", trigger: "blur" },
  ],
  address: [
    { required: true, validator: validateAddress, trigger: "blur" },
  ],
};

watch(
  () => props.visible,
  (val) => {
    if (val) {
      activeTab.value = "basic";
      if (props.server) {
        // 编辑模式：将 host:port 组合成 address
        formData.id = props.server.id;
        formData.name = props.server.name;
        formData.address = `${props.server.host}:${props.server.port}`;
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
      } else {
        // 新增模式：重置表单
        formData.id = undefined;
        formData.name = "";
        formData.address = "";
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

  const { host, port } = parseAddress(formData.address);

  const serverData: MqttServer = {
    id: formData.id,
    name: formData.name,
    host,
    port,
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
</style>
