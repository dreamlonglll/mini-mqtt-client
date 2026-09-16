<template>
  <!-- 组件按需引入后不再全局 app.use(ElementPlus)，组件库语言改由这里下发 -->
  <el-config-provider :locale="elementLocale">
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
      v-if="saveTemplateDialogMounted"
      v-model:visible="showSaveTemplateDialog"
      :template="templateToSave"
      :server-id="activeServerId ?? 0"
      :categories="templateCategories"
      @saved="handleTemplateSaved"
    />

    <!-- 定时发布对话框 -->
    <ScheduledPublishDialog
      v-if="scheduledPublishDialogMounted"
      v-model:visible="showScheduledPublishDialog"
      :server-id="activeServerId ?? 0"
      @running-change="handleScheduledPublishRunningChange"
    />

    <!-- 系统设置对话框 -->
    <SettingsDialog v-if="settingsDialogMounted" v-model:visible="showSettingsDialog" />

    <!-- 脚本管理对话框 -->
    <ScriptDialog
      v-if="scriptDialogMounted"
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
  </el-config-provider>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AppLayout from "@/components/layout/AppLayout.vue";
import MainContent from "@/components/mqtt/MainContent.vue";
import { useLazyDialog } from "@/composables/useLazyDialog";
import { useAppStore } from "@/stores/app";
import { useMqttStore } from "@/stores/mqtt";
import { useServerStore } from "@/stores/server";
import { useTemplateStore, type CommandTemplate } from "@/stores/template";
import { ElMessage } from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import en from "element-plus/es/locale/lang/en";

// 以下弹窗与抽屉都只在用户主动打开时才需要，拆成独立 chunk 不进首屏
const TemplateDrawer = defineAsyncComponent(() => import("@/components/template/TemplateDrawer.vue"));
const EnvDrawer = defineAsyncComponent(() => import("@/components/env/EnvDrawer.vue"));
const TemplateDialog = defineAsyncComponent(() => import("@/components/template/TemplateDialog.vue"));
const ScheduledPublishDialog = defineAsyncComponent(() => import("@/components/mqtt/ScheduledPublishDialog.vue"));
const SettingsDialog = defineAsyncComponent(() => import("@/components/settings/SettingsDialog.vue"));
const ScriptDialog = defineAsyncComponent(() => import("@/components/script/ScriptDialog.vue"));

const { t } = useI18n();

const appStore = useAppStore();
const mqttStore = useMqttStore();
const serverStore = useServerStore();
const templateStore = useTemplateStore();

const activeServerId = computed(() => serverStore.activeServerId);
const templateCategories = computed(() => templateStore.categories);
// 组件库内置文案跟随应用语言（切换语言即时生效，无需重启）
const elementLocale = computed(() => (appStore.actualLocale === "zh-CN" ? zhCn : en));

// 抽屉的内容由 el-drawer 惰性渲染，首次打开时才会加载对应 chunk
const showTemplateDrawer = ref(false);
const showEnvDrawer = ref(false);

// 保存模板对话框
const {
  mounted: saveTemplateDialogMounted,
  visible: showSaveTemplateDialog,
  open: openSaveTemplateDialog,
} = useLazyDialog();
const templateToSave = ref<CommandTemplate | null>(null);

// 定时发布对话框
const {
  mounted: scheduledPublishDialogMounted,
  visible: showScheduledPublishDialog,
  open: openScheduledPublishDialog,
} = useLazyDialog();
const isScheduledPublishRunning = ref(false);

// 系统设置对话框
const {
  mounted: settingsDialogMounted,
  visible: showSettingsDialog,
  open: openSettingsDialog,
} = useLazyDialog();

// 脚本管理对话框
const {
  mounted: scriptDialogMounted,
  visible: showScriptDialog,
  open: openScriptDialog,
} = useLazyDialog();

onMounted(async () => {
  // 初始化语言
  appStore.initLocale();
  // 初始化 MQTT 事件监听
  mqttStore.initListeners();
  // 持久化失败在 GUI 下不可见，后端 emit 后这里弹出提示
  listen<string>("storage-error", (event) => {
    ElMessage.error({
      message: `${t('errors.storageFailed')}: ${event.payload}`,
      duration: 8000,
    });
  });

  // 主窗口在配置里是初始隐藏的：等主题真正应用完再显示，避免启动先闪空白页与浅色背景。
  // 无论主题初始化成败都要显示窗口（Rust 侧另有 3 秒兜底）。
  try {
    await appStore.initTheme();
  } finally {
    getCurrentWindow()
      .show()
      .catch(() => {
        // 纯浏览器开发模式下没有 Tauri 窗口
      });
  }
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
  openSaveTemplateDialog();
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
  openScheduledPublishDialog();
}

// 定时发布运行状态变化
function handleScheduledPublishRunningChange(running: boolean) {
  isScheduledPublishRunning.value = running;
}

// 打开系统设置
function handleOpenSettings() {
  openSettingsDialog();
}

// 打开脚本管理
function handleOpenScripts() {
  if (!activeServerId.value) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }
  openScriptDialog();
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
