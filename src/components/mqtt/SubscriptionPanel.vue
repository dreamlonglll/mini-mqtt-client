<template>
  <div class="subscription-panel app-card">
    <div class="panel-header">
      <span class="panel-title">
        <el-icon><Collection /></el-icon>
        订阅列表
      </span>
      <el-button type="primary" size="small" :icon="Plus" @click="handleAddSubscription">
        新增
      </el-button>
    </div>

    <div class="subscription-list">
      <div
        v-for="sub in subscriptions"
        :key="sub.id"
        class="subscription-item"
        :class="{ active: sub.id === activeSubscriptionId }"
        @click="handleSelectSubscription(sub.id)"
      >
        <div class="sub-main">
          <span class="sub-topic text-ellipsis">{{ sub.topic }}</span>
          <el-tag size="small" effect="plain" class="sub-qos">
            QoS {{ sub.qos }}
          </el-tag>
        </div>
        <div class="sub-actions">
          <el-button
            text
            size="small"
            :icon="Edit"
            @click.stop="handleEditSubscription(sub)"
          />
          <el-button
            text
            size="small"
            type="danger"
            :icon="Delete"
            @click.stop="handleDeleteSubscription(sub.id)"
          />
        </div>
      </div>

      <el-empty
        v-if="subscriptions.length === 0"
        description="暂无订阅"
        :image-size="60"
      >
        <el-button type="primary" size="small" @click="handleAddSubscription">
          添加订阅
        </el-button>
      </el-empty>
    </div>

    <!-- 添加/编辑订阅对话框 -->
    <el-dialog
      v-model="showDialog"
      :title="isEditing ? '编辑订阅' : '添加订阅'"
      width="420px"
      :close-on-click-modal="false"
    >
      <el-form :model="formData" label-width="80px">
        <el-form-item label="Topic">
          <el-input
            v-model="formData.topic"
            placeholder="例如: sensor/+/temperature 或 device/#"
          >
            <template #suffix>
              <el-tooltip placement="top">
                <template #content>
                  <div style="max-width: 250px">
                    <p><strong>通配符说明：</strong></p>
                    <p><code>+</code> 单层通配符，匹配一个层级</p>
                    <p><code>#</code> 多层通配符，匹配多个层级</p>
                  </div>
                </template>
                <el-icon style="cursor: help"><QuestionFilled /></el-icon>
              </el-tooltip>
            </template>
          </el-input>
        </el-form-item>
        <el-form-item label="QoS">
          <el-radio-group v-model="formData.qos">
            <el-radio-button :value="0">QoS 0</el-radio-button>
            <el-radio-button :value="1">QoS 1</el-radio-button>
            <el-radio-button :value="2">QoS 2</el-radio-button>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showDialog = false">取消</el-button>
        <el-button type="primary" @click="handleConfirm">
          {{ isEditing ? '保存' : '订阅' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue";
import {
  Collection,
  Plus,
  Edit,
  Delete,
  QuestionFilled,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";

interface Subscription {
  id: number;
  topic: string;
  qos: number;
}

const subscriptions = ref<Subscription[]>([]);
const activeSubscriptionId = ref<number | null>(null);
const showDialog = ref(false);
const isEditing = ref(false);
const editingId = ref<number | null>(null);

const formData = reactive({
  topic: "",
  qos: 0,
});

let nextId = 1;

const handleSelectSubscription = (id: number) => {
  activeSubscriptionId.value = id;
};

const handleAddSubscription = () => {
  isEditing.value = false;
  editingId.value = null;
  formData.topic = "";
  formData.qos = 0;
  showDialog.value = true;
};

const handleEditSubscription = (sub: Subscription) => {
  isEditing.value = true;
  editingId.value = sub.id;
  formData.topic = sub.topic;
  formData.qos = sub.qos;
  showDialog.value = true;
};

const handleDeleteSubscription = async (id: number) => {
  try {
    await ElMessageBox.confirm("确定要取消此订阅吗？", "确认", {
      confirmButtonText: "确定",
      cancelButtonText: "取消",
      type: "warning",
    });

    const index = subscriptions.value.findIndex((s) => s.id === id);
    if (index !== -1) {
      subscriptions.value.splice(index, 1);
      ElMessage.success("订阅已取消");
    }
  } catch {
    // 用户取消
  }
};

const handleConfirm = () => {
  if (!formData.topic.trim()) {
    ElMessage.warning("请输入 Topic");
    return;
  }

  if (isEditing.value && editingId.value !== null) {
    // 编辑模式
    const sub = subscriptions.value.find((s) => s.id === editingId.value);
    if (sub) {
      sub.topic = formData.topic;
      sub.qos = formData.qos;
      ElMessage.success("订阅已更新");
    }
  } else {
    // 新增模式
    subscriptions.value.push({
      id: nextId++,
      topic: formData.topic,
      qos: formData.qos,
    });
    ElMessage.success("订阅成功");
  }

  showDialog.value = false;
};
</script>

<style scoped lang="scss">
.subscription-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--app-border-color);
  flex-shrink: 0;
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
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s ease;

  &:hover {
    background-color: var(--sidebar-hover);

    .sub-actions {
      opacity: 1;
    }
  }

  &.active {
    background-color: var(--sidebar-active);
  }
}

.sub-main {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  flex: 1;
}

.sub-topic {
  font-size: 13px;
  font-family: "Fira Code", "Consolas", monospace;
  color: var(--app-text-color);
}

.sub-qos {
  align-self: flex-start;
}

.sub-actions {
  display: flex;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
}
</style>
