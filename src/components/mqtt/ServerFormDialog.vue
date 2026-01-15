<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? '编辑 Server' : '新增 Server'"
    width="580px"
    destroy-on-close
    @update:model-value="$emit('update:visible', $event)"
  >
    <el-form
      ref="formRef"
      :model="form"
      :rules="rules"
      label-width="100px"
      label-position="right"
    >
      <el-tabs v-model="activeTab">
        <!-- 基本配置 -->
        <el-tab-pane label="基本配置" name="basic">
          <el-form-item label="名称" prop="name">
            <el-input v-model="form.name" placeholder="给这个连接起个名字" />
          </el-form-item>

          <el-row :gutter="16">
            <el-col :span="16">
              <el-form-item label="主机地址" prop="host">
                <el-input v-model="form.host" placeholder="例如: broker.emqx.io" />
              </el-form-item>
            </el-col>
            <el-col :span="8">
              <el-form-item label="端口" prop="port">
                <el-input-number v-model="form.port" :min="1" :max="65535" style="width: 100%" />
              </el-form-item>
            </el-col>
          </el-row>

          <el-form-item label="协议版本" prop="protocol_version">
            <el-radio-group v-model="form.protocol_version">
              <el-radio value="3.1.1">MQTT 3.1.1</el-radio>
              <el-radio value="5.0">MQTT 5.0</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item label="Client ID" prop="client_id">
            <el-input v-model="form.client_id" placeholder="留空则自动生成">
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
            <el-input v-model="form.username" placeholder="可选" />
          </el-form-item>

          <el-form-item label="密码" prop="password">
            <el-input
              v-model="form.password"
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
                <el-input-number v-model="form.keep_alive" :min="0" :max="65535" style="width: 100%" />
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="Clean Session">
                <el-switch v-model="form.clean_session" />
              </el-form-item>
            </el-col>
          </el-row>

          <el-form-item label="启用 TLS">
            <el-switch v-model="form.use_tls" />
          </el-form-item>

          <template v-if="form.use_tls">
            <el-form-item label="CA 证书" prop="ca_cert">
              <el-input
                v-model="form.ca_cert"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式证书内容"
              />
            </el-form-item>

            <el-form-item label="客户端证书" prop="client_cert">
              <el-input
                v-model="form.client_cert"
                type="textarea"
                :rows="3"
                placeholder="PEM 格式证书内容（可选）"
              />
            </el-form-item>

            <el-form-item label="客户端私钥" prop="client_key">
              <el-input
                v-model="form.client_key"
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
import { ref, watch, computed } from "vue";
import type { FormInstance, FormRules } from "element-plus";
import { RefreshRight } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useServerStore } from "@/stores/server";
import type { MqttServer } from "@/types/mqtt";
import { createDefaultServer } from "@/types/mqtt";

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

const form = ref<MqttServer>(createDefaultServer());

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
        form.value = { ...props.server };
      } else {
        form.value = createDefaultServer();
      }
    }
  }
);

const generateClientId = () => {
  const random = Math.random().toString(36).substring(2, 10);
  form.value.client_id = `mqtt_${Date.now()}_${random}`;
};

const handleSave = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;

  saving.value = true;
  try {
    if (isEdit.value) {
      await serverStore.updateServer(form.value);
      ElMessage.success("更新成功");
    } else {
      await serverStore.createServer(form.value);
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
