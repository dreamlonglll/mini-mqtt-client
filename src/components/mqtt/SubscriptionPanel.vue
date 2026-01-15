<template>
  <div class="subscription-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Collection /></el-icon>
        订阅列表
      </span>
      <el-button type="primary" size="small" :icon="Plus" @click="handleAddSubscription">
        订阅
      </el-button>
    </div>
    
    <div class="subscription-list">
      <div
        v-for="sub in subscriptions"
        :key="sub.id"
        class="subscription-item"
      >
        <div class="sub-info">
          <span class="sub-topic text-ellipsis">{{ sub.topic }}</span>
          <el-tag size="small" effect="plain">QoS {{ sub.qos }}</el-tag>
        </div>
        <el-button
          type="danger"
          text
          size="small"
          :icon="Close"
          @click="handleUnsubscribe(sub.id)"
        />
      </div>
      
      <div v-if="subscriptions.length === 0" class="empty-hint">
        暂无订阅，点击上方按钮添加
      </div>
    </div>
    
    <!-- 添加订阅对话框 -->
    <el-dialog
      v-model="showAddDialog"
      title="添加订阅"
      width="420px"
      :close-on-click-modal="false"
    >
      <el-form :model="newSubscription" label-width="80px">
        <el-form-item label="Topic">
          <el-input
            v-model="newSubscription.topic"
            placeholder="例如: sensor/+/temperature"
          />
        </el-form-item>
        <el-form-item label="QoS">
          <el-radio-group v-model="newSubscription.qos">
            <el-radio :value="0">QoS 0</el-radio>
            <el-radio :value="1">QoS 1</el-radio>
            <el-radio :value="2">QoS 2</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddDialog = false">取消</el-button>
        <el-button type="primary" @click="handleConfirmSubscribe">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue";
import { Collection, Plus, Close } from "@element-plus/icons-vue";

interface Subscription {
  id: number;
  topic: string;
  qos: number;
}

const subscriptions = ref<Subscription[]>([]);
const showAddDialog = ref(false);
const newSubscription = reactive({
  topic: "",
  qos: 0,
});

let nextId = 1;

const handleAddSubscription = () => {
  newSubscription.topic = "";
  newSubscription.qos = 0;
  showAddDialog.value = true;
};

const handleConfirmSubscribe = () => {
  if (newSubscription.topic.trim()) {
    subscriptions.value.push({
      id: nextId++,
      topic: newSubscription.topic,
      qos: newSubscription.qos,
    });
    showAddDialog.value = false;
  }
};

const handleUnsubscribe = (id: number) => {
  const index = subscriptions.value.findIndex((s) => s.id === id);
  if (index !== -1) {
    subscriptions.value.splice(index, 1);
  }
};
</script>

<style scoped lang="scss">
.subscription-panel {
  display: flex;
  flex-direction: column;
  min-height: 120px;
  max-height: 200px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
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

.subscription-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.subscription-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-radius: 6px;
  transition: background-color 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);
  }
}

.sub-info {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.sub-topic {
  font-size: 13px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
}

.empty-hint {
  text-align: center;
  padding: 20px;
  font-size: 13px;
  color: var(--app-text-secondary);
}
</style>
