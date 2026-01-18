<template>
  <AppLayout @open-templates="handleOpenTemplates" @open-scripts="handleOpenScripts" @open-env="handleOpenEnv" @settings="handleOpenSettings">
    <!-- 消息调试视图 -->
    <MainContent 
      :scheduled-publish-running="isScheduledPublishRunning"
      @save-template="handleSaveTemplate" 
      @open-templates="handleOpenTemplates"
      @scheduled-publish="handleScheduledPublish"
    />
  </AppLayout>

  <!-- 模板管理抽屉 -->
  <el-drawer
    v-model="showTemplateDrawer"
    :title="$t('template.drawerTitle')"
    direction="rtl"
    size="480px"
    :close-on-click-modal="true"
  >
    <TemplateDrawer 
      v-if="activeServerId"
      :server-id="activeServerId"
      @use="handleUseTemplate"
    />
  </el-drawer>

  <!-- 保存模板对话框 -->
  <TemplateDialog
    v-model:visible="showSaveTemplateDialog"
    :template="templateToSave"
    :server-id="activeServerId ?? 0"
    :categories="templateCategories"
    @saved="handleTemplateSaved"
  />

  <!-- 定时发布对话框 -->
  <ScheduledPublishDialog
    v-model:visible="showScheduledPublishDialog"
    :server-id="activeServerId ?? 0"
    @running-change="handleScheduledPublishRunningChange"
  />

  <!-- 系统设置对话框 -->
  <SettingsDialog v-model:visible="showSettingsDialog" />

  <!-- 脚本管理对话框 -->
  <ScriptDialog
    v-model:visible="showScriptDialog"
    :server-id="activeServerId ?? 0"
  />

  <!-- 环境变量抽屉 -->
  <el-drawer
    v-model="showEnvDrawer"
    :title="$t('env.drawerTitle')"
    direction="rtl"
    size="480px"
    :close-on-click-modal="true"
  >
    <EnvDrawer 
      v-if="activeServerId"
      :server-id="activeServerId"
    />
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import AppLayout from "@/components/layout/AppLayout.vue";
import MainContent from "@/components/mqtt/MainContent.vue";
import TemplateDrawer from "@/components/template/TemplateDrawer.vue";
import TemplateDialog from "@/components/template/TemplateDialog.vue";
import ScheduledPublishDialog from "@/components/mqtt/ScheduledPublishDialog.vue";
import SettingsDialog from "@/components/settings/SettingsDialog.vue";
import ScriptDialog from "@/components/script/ScriptDialog.vue";
import EnvDrawer from "@/components/env/EnvDrawer.vue";
import { useAppStore } from "@/stores/app";
import { useMqttStore } from "@/stores/mqtt";
import { useServerStore } from "@/stores/server";
import { useTemplateStore, type CommandTemplate } from "@/stores/template";
import { ElMessage } from "element-plus";

const { t } = useI18n();

const appStore = useAppStore();
const mqttStore = useMqttStore();
const serverStore = useServerStore();
const templateStore = useTemplateStore();

const activeServerId = computed(() => serverStore.activeServerId);
const templateCategories = computed(() => templateStore.categories);

// 模板管理抽屉
const showTemplateDrawer = ref(false);

// 保存模板对话框
const showSaveTemplateDialog = ref(false);
const templateToSave = ref<CommandTemplate | null>(null);

// 定时发布对话框
const showScheduledPublishDialog = ref(false);
const isScheduledPublishRunning = ref(false);

// 系统设置对话框
const showSettingsDialog = ref(false);

// 脚本管理对话框
const showScriptDialog = ref(false);

// 环境变量抽屉
const showEnvDrawer = ref(false);

onMounted(() => {
  // 初始化主题
  appStore.initTheme();
  // 初始化语言
  appStore.initLocale();
  // 初始化 MQTT 事件监听
  mqttStore.initListeners();
});

// 处理保存模板请求
function handleSaveTemplate(data: { topic: string; payload: string; qos: number; retain: boolean; payloadType: string }) {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  
  // 创建临时模板对象用于对话框
  templateToSave.value = {
    server_id: activeServerId.value,
    name: "",
    topic: data.topic,
    payload: data.payload,
    payload_type: data.payloadType as 'json' | 'text' | 'hex',
    qos: data.qos as 0 | 1 | 2,
    retain: data.retain,
    use_count: 0,
  };
  
  // 加载分类列表
  templateStore.loadCategories(activeServerId.value);
  showSaveTemplateDialog.value = true;
}

// 模板保存成功
function handleTemplateSaved() {
  showSaveTemplateDialog.value = false;
  templateToSave.value = null;
  ElMessage.success(t('template.saveSuccess'));
}

// 打开模板管理抽屉
function handleOpenTemplates() {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  showTemplateDrawer.value = true;
}

// 使用模板
function handleUseTemplate(template: CommandTemplate) {
  // 复制到发布面板
  appStore.setCopyToPublish({
    topic: template.topic,
    payload: template.payload,
    qos: template.qos,
    retain: template.retain,
    payloadType: template.payload_type,
  });
  // 关闭抽屉
  showTemplateDrawer.value = false;
  ElMessage.success(`${t('template.loadSuccess')}: ${template.name}`);
}

// 打开定时发布对话框
function handleScheduledPublish() {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  showScheduledPublishDialog.value = true;
}

// 定时发布运行状态变化
function handleScheduledPublishRunningChange(running: boolean) {
  isScheduledPublishRunning.value = running;
}

// 打开系统设置
function handleOpenSettings() {
  showSettingsDialog.value = true;
}

// 打开脚本管理
function handleOpenScripts() {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  showScriptDialog.value = true;
}

// 打开环境变量管理
function handleOpenEnv() {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  showEnvDrawer.value = true;
}
</script>

<style>
/* 可在此添加额外的全局样式 */
</style>
