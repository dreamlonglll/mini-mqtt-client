<template>
  <SidebarSection :title="$t('sidebar.subscription')">
    <template #action>
      <el-button type="primary" size="small" :icon="Plus" circle @click="handleAdd" />
    </template>

    <div class="subscription-list">
      <div
        v-for="sub in subscriptions"
        :key="sub.id"
        class="subscription-item"
        :class="{ inactive: !sub.is_active }"
      >
        <!-- 第一行：颜色 + Topic + 开关 -->
        <div class="sub-row-1">
          <span
            v-if="sub.color"
            class="sub-color-indicator"
            :style="{ backgroundColor: sub.color }"
          />
          <el-tooltip :content="sub.topic" placement="top" :show-after="500">
            <span class="sub-topic text-ellipsis">{{ sub.topic }}</span>
          </el-tooltip>
          <el-switch
            :model-value="sub.is_active"
            size="small"
            @change="(val: string | number | boolean) => handleToggle(sub, Boolean(val))"
          />
        </div>
        <!-- 第二行：QoS + 操作按钮 -->
        <div class="sub-row-2">
          <el-tag size="small" effect="plain" type="info">Q{{ sub.qos }}</el-tag>
          <div class="sub-actions">
            <el-button text size="small" :icon="Edit" @click="handleEdit(sub)" />
            <el-button text size="small" type="danger" :icon="Close" @click="handleDelete(sub)" />
          </div>
        </div>
      </div>

      <div v-if="subscriptions.length === 0" class="empty-hint">
        {{ $t('sidebar.addSubscriptionHint') }}
      </div>
    </div>
  </SidebarSection>

  <SubscriptionDialog
    v-model:visible="showDialog"
    :server-id="serverStore.activeServerId"
    :subscription="editingSubscription"
  />
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Plus, Edit, Close } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useServerStore } from "@/stores/server";
import { useSubscriptionStore } from "@/stores/subscription";
import SidebarSection from "./SidebarSection.vue";
import SubscriptionDialog from "./SubscriptionDialog.vue";
import type { Subscription } from "@/types/mqtt";

const { t } = useI18n();
const serverStore = useServerStore();
const subscriptionStore = useSubscriptionStore();

const showDialog = ref(false);
const editingSubscription = ref<Subscription | null>(null);

const subscriptions = computed(() => {
  const serverId = serverStore.activeServerId;
  if (!serverId) return [];
  return subscriptionStore.getSubscriptionsByServer(serverId);
});

// 切换服务器时加载对应的订阅列表
watch(
  () => serverStore.activeServerId,
  (serverId) => {
    if (serverId) {
      subscriptionStore.fetchSubscriptions(serverId);
    }
  },
  { immediate: true }
);

const handleAdd = () => {
  editingSubscription.value = null;
  showDialog.value = true;
};

const handleEdit = (sub: Subscription) => {
  editingSubscription.value = sub;
  showDialog.value = true;
};

const handleDelete = async (sub: Subscription) => {
  try {
    await ElMessageBox.confirm(t('sidebar.deleteSubscriptionConfirm', { topic: sub.topic }), t('common.confirm'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: "warning",
    });
    await subscriptionStore.removeSubscription(
      sub.id!,
      serverStore.activeServerId!,
      sub.topic
    );
    ElMessage.success(t('success.unsubscribed'));
  } catch (error) {
    if (error !== "cancel") {
      ElMessage.error(`${t('errors.unsubscribeFailed')}: ${error}`);
    }
  }
};

const handleToggle = async (sub: Subscription, isActive: boolean) => {
  try {
    await subscriptionStore.toggleSubscription(
      sub.id!,
      serverStore.activeServerId!,
      sub.topic,
      sub.qos,
      isActive
    );
    ElMessage.success(isActive ? t('success.resumed') : t('success.paused'));
  } catch (error) {
    ElMessage.error(`${t('errors.subscribeFailed')}: ${error}`);
  }
};
</script>

<style scoped lang="scss">
.subscription-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.subscription-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid var(--app-border-color);
  transition: all 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);
    border-color: var(--el-color-primary-light-5);

    .sub-actions {
      opacity: 1;
    }
  }

  &.inactive {
    opacity: 0.6;
  }
}

.sub-row-1 {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;

  .el-switch {
    flex-shrink: 0;
    margin-left: auto;
  }
}

.sub-row-2 {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.sub-color-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.sub-topic {
  font-size: 12px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
  flex: 1;
  min-width: 0;
}

.sub-actions {
  display: flex;
  gap: 0;
  opacity: 0;
  transition: opacity 0.2s ease;

  .el-button {
    padding: 4px;
  }
}

.empty-hint {
  text-align: center;
  padding: 12px;
  font-size: 12px;
  color: var(--app-text-secondary);
}
</style>
