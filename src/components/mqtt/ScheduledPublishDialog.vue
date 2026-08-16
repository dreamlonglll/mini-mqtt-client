<template>
  <el-dialog
    v-model="dialogVisible"
    :title="isRunning ? `${$t('scheduled.title')} - ${$t('scheduled.running')}` : (isCompleted ? `${$t('scheduled.title')}` : $t('scheduled.title'))"
    width="700px"
    :close-on-click-modal="false"
    :close-on-press-escape="!isRunning"
    @close="handleClose"
  >
    <!-- 配置视图 -->
    <div v-if="!isRunning && !isCompleted" class="config-view">
      <!-- 命令列表 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">{{ $t('scheduled.selectTemplate') }}</span>
          <div class="section-actions">
            <el-checkbox
              v-model="selectAll"
              :indeterminate="isIndeterminate"
              @change="(val: boolean | string | number) => handleSelectAll(Boolean(val))"
            >
              {{ $t('template.allCategories') }}
            </el-checkbox>
            <span class="selected-count">{{ $t('scheduled.selectedTemplates', { count: selectedIds.length }) }}</span>
          </div>
        </div>
        <div class="command-list" v-loading="loading">
          <div
            v-for="cmd in templates"
            :key="cmd.id"
            class="command-item"
            :class="{ selected: selectedIds.includes(cmd.id!) }"
            @click="toggleSelect(cmd.id!)"
          >
            <el-checkbox
              :model-value="selectedIds.includes(cmd.id!)"
              @change="toggleSelect(cmd.id!)"
              @click.stop
            />
            <div class="command-info">
              <div class="command-name">{{ cmd.name }}</div>
              <div class="command-meta">
                <span class="topic">
                  <el-icon><Position /></el-icon>
                  {{ cmd.topic }}
                </span>
                <span class="payload">{{ truncatePayload(cmd.payload) }}</span>
                <el-tag size="small" :type="getPayloadTypeTag(cmd.payload_type)">
                  {{ cmd.payload_type.toUpperCase() }}
                </el-tag>
                <el-tag size="small" type="info">QoS {{ cmd.qos }}</el-tag>
              </div>
            </div>
          </div>
          <el-empty v-if="templates.length === 0" :description="$t('template.noTemplate')" :image-size="60" />
        </div>
      </div>

      <!-- 发送配置 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">{{ $t('scheduled.mode') }}</span>
        </div>
        <el-form :model="config" label-width="100px" class="config-form">
          <el-row :gutter="16">
            <el-col :span="12">
              <el-form-item :label="$t('scheduled.intervalSeconds')">
                <el-input-number
                  v-model="config.interval"
                  :min="100"
                  :max="60000"
                  :step="100"
                  style="width: 140px"
                />
                <span class="unit">ms</span>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item :label="$t('scheduled.roundInterval')">
                <el-input-number
                  v-model="config.roundInterval"
                  :min="0"
                  :max="60000"
                  :step="100"
                  style="width: 140px"
                />
                <span class="unit">ms</span>
              </el-form-item>
            </el-col>
          </el-row>
          <el-form-item :label="$t('scheduled.order')">
            <el-radio-group v-model="config.order">
              <el-radio value="selection">{{ $t('scheduled.selectionOrder') }}</el-radio>
              <el-radio value="name">{{ $t('scheduled.nameOrder') }}</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item :label="$t('scheduled.loopMode')">
            <el-radio-group v-model="config.loopMode">
              <el-radio value="infinite">{{ $t('scheduled.infinite') }}</el-radio>
              <el-radio value="count">{{ $t('scheduled.count') }}</el-radio>
            </el-radio-group>
            <el-input-number
              v-if="config.loopMode === 'count'"
              v-model="config.loopCount"
              :min="1"
              :max="9999"
              style="margin-left: 12px; width: 100px"
            />
          </el-form-item>
        </el-form>
      </div>
    </div>

    <!-- 运行视图（运行中或已完成时显示） -->
    <div v-else class="running-view">
      <!-- 发送状态 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">{{ $t('scheduled.status') }}</span>
        </div>
        <div class="status-panel">
          <div class="status-indicator" :class="{ running: isRunning, completed: isCompleted }">
            <el-icon v-if="isRunning" class="spinning"><Loading /></el-icon>
            <el-icon v-else><SuccessFilled /></el-icon>
            <span>{{ isRunning ? $t('scheduled.running') : $t('scheduled.completed') }}</span>
          </div>
          <div class="status-info">
            <div class="info-row">
              <span class="label">{{ $t('scheduled.topic') }}:</span>
              <code class="value">{{ currentCommand?.topic || '-' }}</code>
            </div>
            <div class="info-row">
              <span class="label">{{ $t('scheduled.round') }}:</span>
              <span class="value">{{ currentRound }}</span>
            </div>
            <div class="info-row">
              <span class="label">{{ $t('scheduled.sent') }}:</span>
              <span class="value">{{ sentCount }}</span>
            </div>
            <div class="info-row">
              <span class="label">{{ $t('scheduled.successFail') }}:</span>
              <span class="value success">{{ successCount }}</span>
              <span> / </span>
              <span class="value error">{{ failCount }}</span>
            </div>
          </div>
          <el-progress
            :percentage="progressPercentage"
            :format="() => progressText"
            :stroke-width="10"
          />
        </div>
      </div>

      <!-- 发送日志 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">{{ $t('scheduled.logs') }}</span>
          <el-button text size="small" @click="clearLogs">{{ $t('messages.clear') }}</el-button>
        </div>
        <div class="log-list" ref="logListRef">
          <div
            v-for="(log, index) in logs"
            :key="index"
            class="log-item"
            :class="log.status"
          >
            <div class="log-header">
              <span class="log-time">[{{ log.time }}]</span>
              <span class="log-topic">{{ log.topic }}</span>
              <el-tag :type="log.status === 'success' ? 'success' : 'danger'" size="small">
                {{ log.status === 'success' ? 'OK' : 'Fail' }}
              </el-tag>
            </div>
            <div class="log-payload">
              <code>{{ truncatePayload(log.payload) }}</code>
            </div>
          </div>
          <div v-if="logs.length === 0" class="empty-log">{{ $t('messages.noMessages') }}</div>
        </div>
      </div>
    </div>

    <template #footer>
      <!-- 配置视图按钮 -->
      <template v-if="!isRunning && !isCompleted">
        <el-button @click="handleClose">{{ $t('common.cancel') }}</el-button>
        <el-button
          type="primary"
          :disabled="selectedIds.length === 0"
          @click="handleStart"
        >
          {{ $t('scheduled.start') }}
        </el-button>
      </template>
      <!-- 运行中按钮 -->
      <template v-else-if="isRunning">
        <el-button @click="handleMinimize">{{ $t('scheduled.minimize') }}</el-button>
        <el-button type="danger" @click="handleStop">
          {{ $t('scheduled.stop') }}
        </el-button>
      </template>
      <!-- 完成后按钮 -->
      <template v-else>
        <el-button @click="handleBackToConfig">{{ $t('scheduled.back') }}</el-button>
        <el-button type="primary" @click="handleClose">{{ $t('common.close') }}</el-button>
      </template>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { Position, Loading, SuccessFilled } from '@element-plus/icons-vue'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'
import { useScheduledPublisher } from '@/composables/useScheduledPublisher'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  serverId: number
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  'running-change': [isRunning: boolean]
}>()

const templateStore = useTemplateStore()

// 调度状态机：节奏、轮次、计数与日志都在这里，组件只管选命令和显示
const {
  config,
  isRunning,
  isCompleted,
  currentCommand,
  currentRound,
  sentCount,
  successCount,
  failCount,
  logs,
  progressPercentage,
  progressText,
  start: startPublishing,
  stop: stopPublishing,
  reset: resetPublisher,
  clearLogs,
} = useScheduledPublisher({
  serverId: () => props.serverId,
  onLog: scheduleLogScroll,
  onFinished: () => ElMessage.success(t('success.published')),
})

// 对话框可见性
const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit('update:visible', val)
})

// 加载状态
const loading = computed(() => templateStore.loading)
const templates = computed(() => templateStore.templates)

// 选择状态
const selectedIds = ref<number[]>([])
const selectAll = ref(false)
const isIndeterminate = computed(() => {
  return selectedIds.value.length > 0 && selectedIds.value.length < templates.value.length
})

// 是否已最小化（用于区分最小化和关闭）
const isMinimized = ref(false)

const logListRef = ref<HTMLElement | null>(null)

// 监听对话框打开
watch(() => props.visible, (visible) => {
  if (visible && props.serverId) {
    templateStore.loadTemplates(props.serverId)
    // 如果不是运行中（从最小化恢复），则重置状态
    if (!isRunning.value) {
      resetState()
    }
  }
})

// 重置状态
function resetState() {
  selectedIds.value = []
  selectAll.value = false
  isMinimized.value = false
  resetPublisher()
}

// 获取payload类型标签颜色
function getPayloadTypeTag(type: string) {
  switch (type) {
    case 'json': return 'primary'
    case 'hex': return 'warning'
    default: return 'info'
  }
}

// 截断payload
function truncatePayload(payload: string): string {
  const maxLen = 50
  if (payload.length > maxLen) {
    return payload.substring(0, maxLen) + '...'
  }
  return payload
}

// 切换选择
function toggleSelect(id: number) {
  const index = selectedIds.value.indexOf(id)
  if (index === -1) {
    selectedIds.value.push(id)
  } else {
    selectedIds.value.splice(index, 1)
  }
  updateSelectAll()
}

// 更新全选状态
function updateSelectAll() {
  selectAll.value = selectedIds.value.length === templates.value.length && templates.value.length > 0
}

// 全选处理
function handleSelectAll(val: boolean) {
  if (val) {
    selectedIds.value = templates.value.map(t => t.id!)
  } else {
    selectedIds.value = []
  }
}

// 获取排序后的命令
function getOrderedCommands(): CommandTemplate[] {
  const selected = templates.value.filter(t => selectedIds.value.includes(t.id!))
  if (config.value.order === 'name') {
    return [...selected].sort((a, b) => a.name.localeCompare(b.name))
  }
  // 按勾选顺序
  return selectedIds.value.map(id => selected.find(t => t.id === id)!).filter(Boolean)
}

// 日志滚动合并到 rAF，避免高频发布时每条日志都强制同步 reflow
let logScrollScheduled = false
function scheduleLogScroll() {
  if (logScrollScheduled) return
  logScrollScheduled = true
  requestAnimationFrame(() => {
    logScrollScheduled = false
    if (logListRef.value) {
      logListRef.value.scrollTop = logListRef.value.scrollHeight
    }
  })
}

// 运行状态的每次变化都通知父组件（父组件据此在主界面显示"定时发布中"）
watch(isRunning, (running) => emit('running-change', running))

// 开始发布
async function handleStart() {
  if (selectedIds.value.length === 0) {
    ElMessage.warning(t('scheduled.noTemplateSelected'))
    return
  }
  await startPublishing(getOrderedCommands())
}

// 停止发布
function handleStop() {
  stopPublishing(true)
  ElMessage.info(t('scheduled.stop'))
}

// 返回配置界面
function handleBackToConfig() {
  resetPublisher()
}

// 最小化对话框：只关窗口，发布继续在后台跑
function handleMinimize() {
  isMinimized.value = true
  dialogVisible.value = false
}

// 对话框关闭事件（由 el-dialog 的 @close 触发或取消按钮调用）
function handleClose() {
  // 如果是最小化操作，不停止发布
  if (isMinimized.value) {
    isMinimized.value = false
    return
  }
  // 正常关闭时停止发布
  if (isRunning.value) {
    stopPublishing()
  }
  dialogVisible.value = false
}
</script>

<style scoped lang="scss">
.config-view,
.running-view {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.section {
  background-color: var(--sidebar-bg);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  overflow: hidden;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background-color: var(--card-bg);
  border-bottom: 1px solid var(--app-border-color);
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.section-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.selected-count {
  font-size: 12px;
  color: var(--app-text-secondary);
}

.command-list {
  max-height: 240px;
  overflow-y: auto;
}

.command-item {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--app-border-color);
  cursor: pointer;
  transition: background-color 0.2s;

  &:last-child {
    border-bottom: none;
  }

  &:hover {
    background-color: var(--sidebar-hover);
  }

  &.selected {
    background-color: var(--primary-light);
  }
}

.command-info {
  flex: 1;
  min-width: 0;
}

.command-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--app-text-color);
  margin-bottom: 6px;
}

.command-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--app-text-secondary);

  .topic {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .payload {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'Fira Code', 'Consolas', monospace;
  }
}

.config-form {
  padding: 16px;
}

.unit {
  margin-left: 8px;
  font-size: 12px;
  color: var(--app-text-secondary);
}

// 运行视图
.status-panel {
  padding: 20px;
}

.status-indicator {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  font-weight: 500;
  color: var(--status-connected);
  margin-bottom: 16px;
  white-space: nowrap;

  .spinning {
    animation: spin 1s linear infinite;
    flex-shrink: 0;
  }

  &.completed {
    color: var(--el-color-success);
  }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.status-info {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}

.info-row {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  white-space: nowrap;

  .label {
    font-size: 13px;
    color: var(--app-text-secondary);
    flex-shrink: 0;
  }

  .value {
    font-size: 13px;
    font-weight: 500;
    color: var(--app-text-color);

    &.success {
      color: var(--status-connected);
    }

    &.error {
      color: var(--status-error);
    }
  }

  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    background-color: var(--sidebar-hover);
    padding: 2px 6px;
    border-radius: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
}

.log-list {
  max-height: 200px;
  overflow-y: auto;
  padding: 8px 16px;
}

.log-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0;
  font-size: 12px;
  border-bottom: 1px dashed var(--app-border-color);

  &:last-child {
    border-bottom: none;
  }
}

.log-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.log-time {
  color: var(--app-text-secondary);
  font-family: 'Fira Code', 'Consolas', monospace;
  flex-shrink: 0;
}

.log-topic {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--app-text-color);
}

.log-payload {
  margin-left: 8px;
  padding: 4px 8px;
  background-color: var(--sidebar-hover);
  border-radius: 4px;
  
  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 11px;
    color: var(--app-text-secondary);
    word-break: break-all;
  }
}

.empty-log {
  text-align: center;
  padding: 20px;
  color: var(--app-text-secondary);
  font-size: 13px;
}
</style>
