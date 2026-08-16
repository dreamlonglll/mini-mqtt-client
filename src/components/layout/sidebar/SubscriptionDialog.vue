<template>
  <el-dialog
    v-model="dialogVisible"
    :title="isEditing ? $t('sidebar.editSubscription') : $t('sidebar.addSubscription')"
    width="420px"
    :close-on-click-modal="false"
  >
    <el-form :model="formData" label-width="80px">
      <el-form-item :label="$t('sidebar.topic')">
        <el-input v-model="formData.topic" placeholder="e.g., sensor/+/temperature" />
      </el-form-item>
      <el-form-item :label="$t('publish.qos')">
        <el-radio-group v-model="formData.qos">
          <el-radio-button :value="0">QoS 0</el-radio-button>
          <el-radio-button :value="1">QoS 1</el-radio-button>
          <el-radio-button :value="2">QoS 2</el-radio-button>
        </el-radio-group>
      </el-form-item>
      <el-form-item :label="$t('sidebar.colorMark')">
        <div class="color-picker-container">
          <div class="color-options">
            <div
              v-for="color in COLOR_OPTIONS"
              :key="color"
              class="color-option"
              :class="{ active: formData.color === color }"
              :style="{ backgroundColor: color }"
              @click="formData.color = color"
            />
            <div
              class="color-option no-color"
              :class="{ active: !formData.color }"
              @click="formData.color = ''"
              :title="$t('sidebar.noColor')"
            >
              <el-icon><Close /></el-icon>
            </div>
          </div>
          <el-color-picker
            v-model="formData.color"
            size="small"
            show-alpha
            :predefine="COLOR_OPTIONS"
          />
        </div>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="dialogVisible = false">{{ $t('common.cancel') }}</el-button>
      <el-button type="primary" :loading="loading" @click="handleConfirm">
        {{ isEditing ? $t('common.save') : $t('sidebar.subscribe') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Close } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { useSubscriptionStore } from "@/stores/subscription";
import type { Subscription } from "@/types/mqtt";

const props = defineProps<{
  visible: boolean;
  serverId: number | null;
  /** 传入订阅表示编辑，null 表示新增 */
  subscription: Subscription | null;
}>();

const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const { t } = useI18n();
const subscriptionStore = useSubscriptionStore();

/** 预设颜色选项：红 橙 黄 绿 蓝 紫 粉 青 */
const COLOR_OPTIONS = [
  "#F56C6C",
  "#E6A23C",
  "#F2D849",
  "#67C23A",
  "#409EFF",
  "#9B59B6",
  "#FF69B4",
  "#00CED1",
];

const dialogVisible = computed({
  get: () => props.visible,
  set: (value: boolean) => emit("update:visible", value),
});

const isEditing = computed(() => props.subscription !== null);

const loading = ref(false);
const formData = reactive({ topic: "", qos: 0, color: "" });
/** 编辑前的 topic：更新订阅需要用它退订旧 topic */
const originalTopic = ref("");

// 打开时按当前模式初始化表单（pre-flush，在对话框内容渲染前完成，不会闪现上一次的值）
watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    formData.topic = props.subscription?.topic ?? "";
    formData.qos = props.subscription?.qos ?? 0;
    formData.color = props.subscription?.color ?? "";
    originalTopic.value = props.subscription?.topic ?? "";
  }
);

const handleConfirm = async () => {
  if (!formData.topic.trim()) {
    ElMessage.warning(t('errors.inputTopic'));
    return;
  }

  const serverId = props.serverId;
  if (!serverId) {
    ElMessage.warning(t('errors.selectServer'));
    return;
  }

  loading.value = true;
  try {
    if (isEditing.value && props.subscription?.id) {
      await subscriptionStore.updateSubscription(serverId, originalTopic.value, {
        id: props.subscription.id,
        topic: formData.topic,
        qos: formData.qos,
        color: formData.color || undefined,
      });
      ElMessage.success(t('success.saved'));
    } else {
      const newSub = await subscriptionStore.addSubscription(
        serverId,
        formData.topic,
        formData.qos
      );
      // 如果设置了颜色，需要再更新一次
      if (formData.color && newSub.id) {
        await subscriptionStore.updateSubscription(serverId, formData.topic, {
          id: newSub.id,
          color: formData.color,
        });
      }
      ElMessage.success(t('success.subscribed'));
    }
    dialogVisible.value = false;
  } catch (error) {
    console.error("Subscribe failed:", error);
    ElMessage.error(`${t('errors.subscribeFailed')}: ${error}`);
  } finally {
    loading.value = false;
  }
};
</script>

<style scoped lang="scss">
.color-picker-container {
  display: flex;
  align-items: center;
  gap: 12px;
}

.color-options {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.color-option {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  cursor: pointer;
  border: 2px solid transparent;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;

  &:hover {
    transform: scale(1.1);
  }

  &.active {
    border-color: var(--app-text-color);
    box-shadow: 0 0 0 2px var(--sidebar-bg);
  }

  &.no-color {
    background-color: var(--sidebar-bg);
    border: 1px dashed var(--app-border-color);

    .el-icon {
      font-size: 12px;
      color: var(--app-text-secondary);
    }
  }
}
</style>
