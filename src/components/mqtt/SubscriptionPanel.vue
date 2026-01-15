<template>
  <div class="subscription-panel app-card">
    <div class="panel-content">
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
        <el-tag
          v-for="sub in subscriptions"
          :key="sub.id"
          closable
          size="default"
          effect="plain"
          class="sub-tag"
          @close="handleUnsubscribe(sub.id)"
        >
          <span class="sub-topic">{{ sub.topic }}</span>
          <span class="sub-qos">Q{{ sub.qos }}</span>
        </el-tag>
        
        <span v-if="subscriptions.length === 0" class="empty-hint">
          暂无订阅
        </span>
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
import { Collection, Plus } from "@element-plus/icons-vue";

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
}

.panel-content {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 10px 16px;
}

.panel-header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.panel-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
  white-space: nowrap;
}

.subscription-list {
  flex: 1;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  overflow-x: auto;
  min-height: 32px;

  &::-webkit-scrollbar {
    height: 4px;
  }
}

.sub-tag {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 280px;
}

.sub-topic {
  font-family: "Fira Code", "Consolas", monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub-qos {
  font-size: 10px;
  font-weight: 600;
  color: var(--app-text-secondary);
  background: var(--app-bg-color);
  padding: 1px 4px;
  border-radius: 3px;
}

.empty-hint {
  font-size: 13px;
  color: var(--app-text-secondary);
}
</style>
