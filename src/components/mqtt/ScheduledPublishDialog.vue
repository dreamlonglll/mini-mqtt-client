<template>
  <el-dialog
    v-model="dialogVisible"
    :title="isRunning ? '定时发布 - 运行中' : (isCompleted ? '定时发布 - 已完成' : '定时发布')"
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
          <span class="section-title">命令列表</span>
          <div class="section-actions">
            <el-checkbox
              v-model="selectAll"
              :indeterminate="isIndeterminate"
              @change="(val: boolean | string | number) => handleSelectAll(Boolean(val))"
            >
              全选
            </el-checkbox>
            <span class="selected-count">已选择 {{ selectedIds.length }} 条命令</span>
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
          <el-empty v-if="templates.length === 0" description="暂无保存的命令模板" :image-size="60" />
        </div>
      </div>

      <!-- 发送配置 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">发送配置</span>
        </div>
        <el-form :model="config" label-width="100px" class="config-form">
          <el-row :gutter="16">
            <el-col :span="12">
              <el-form-item label="发送间隔">
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
              <el-form-item label="每轮间隔">
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
          <el-form-item label="发送顺序">
            <el-radio-group v-model="config.order">
              <el-radio value="selection">按勾选顺序</el-radio>
              <el-radio value="name">按名称排序</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="循环模式">
            <el-radio-group v-model="config.loopMode">
              <el-radio value="infinite">无限循环</el-radio>
              <el-radio value="count">指定次数</el-radio>
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
          <span class="section-title">发送状态</span>
        </div>
        <div class="status-panel">
          <div class="status-indicator" :class="{ running: isRunning, completed: isCompleted }">
            <el-icon v-if="isRunning" class="spinning"><Loading /></el-icon>
            <el-icon v-else><SuccessFilled /></el-icon>
            <span>{{ isRunning ? '正在发送...' : '发布完成' }}</span>
          </div>
          <div class="status-info">
            <div class="info-row">
              <span class="label">当前命令:</span>
              <code class="value">{{ currentCommand?.topic || '-' }}</code>
            </div>
            <div class="info-row">
              <span class="label">当前轮次:</span>
              <span class="value">第 {{ currentRound }} 轮</span>
            </div>
            <div class="info-row">
              <span class="label">已发送:</span>
              <span class="value">{{ sentCount }} 条消息</span>
            </div>
            <div class="info-row">
              <span class="label">成功/失败:</span>
              <span class="value success">{{ successCount }}</span>
              <span> / </span>
              <span class="value error">{{ failCount }}</span>
            </div>
          </div>
          <el-progress
            :percentage="progressPercentage"
            :format="formatProgress"
            :stroke-width="10"
          />
        </div>
      </div>

      <!-- 发送日志 -->
      <div class="section">
        <div class="section-header">
          <span class="section-title">发送日志</span>
          <el-button text size="small" @click="logs = []">清空</el-button>
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
                {{ log.status === 'success' ? '成功' : '失败' }}
              </el-tag>
            </div>
            <div class="log-payload">
              <code>{{ truncatePayload(log.payload) }}</code>
            </div>
          </div>
          <div v-if="logs.length === 0" class="empty-log">暂无发送记录</div>
        </div>
      </div>
    </div>

    <template #footer>
      <!-- 配置视图按钮 -->
      <template v-if="!isRunning && !isCompleted">
        <el-button @click="handleClose">取消</el-button>
        <el-button
          type="primary"
          :disabled="selectedIds.length === 0"
          @click="handleStart"
        >
          开始发布
        </el-button>
      </template>
      <!-- 运行中按钮 -->
      <template v-else-if="isRunning">
        <el-button @click="handleMinimize">最小化</el-button>
        <el-button type="danger" @click="handleStop">
          停止发布
        </el-button>
      </template>
      <!-- 完成后按钮 -->
      <template v-else>
        <el-button @click="handleBackToConfig">返回配置</el-button>
        <el-button type="primary" @click="handleClose">关闭</el-button>
      </template>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Position, Loading, SuccessFilled } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'
import { useMqttStore } from '@/stores/mqtt'
import { ScriptEngine } from '@/utils/scriptEngine'
import type { Script } from '@/stores/script'

const props = defineProps<{
  visible: boolean
  serverId: number
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  'running-change': [isRunning: boolean]
}>()

const templateStore = useTemplateStore()
const mqttStore = useMqttStore()

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

// 发送配置
const config = ref({
  interval: 1000,
  roundInterval: 0,
  order: 'selection' as 'selection' | 'name',
  loopMode: 'infinite' as 'infinite' | 'count',
  loopCount: 10
})

// 运行状态
const isRunning = ref(false)
const isCompleted = ref(false)  // 是否已完成（用于保持在发布视图）
const isMinimized = ref(false)  // 是否已最小化（用于区分最小化和关闭）
const currentCommand = ref<CommandTemplate | null>(null)
const currentIndex = ref(0)
const currentRound = ref(1)
const sentCount = ref(0)
const successCount = ref(0)
const failCount = ref(0)

// 日志
interface LogEntry {
  time: string
  topic: string
  payload: string
  status: 'success' | 'error'
  message?: string
}
const logs = ref<LogEntry[]>([])
const logListRef = ref<HTMLElement | null>(null)

// 定时器
let publishTimeout: ReturnType<typeof setTimeout> | null = null

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

// 组件卸载时停止
onUnmounted(() => {
  stopPublishing()
})

// 进度百分比
const progressPercentage = computed(() => {
  const total = selectedIds.value.length
  if (total === 0) return 0
  return Math.round((currentIndex.value / total) * 100)
})

// 格式化进度
function formatProgress(_percentage: number) {
  return `${currentIndex.value}/${selectedIds.value.length}`
}

// 重置状态
function resetState() {
  selectedIds.value = []
  selectAll.value = false
  isRunning.value = false
  isCompleted.value = false
  isMinimized.value = false
  currentCommand.value = null
  currentIndex.value = 0
  currentRound.value = 1
  sentCount.value = 0
  successCount.value = 0
  failCount.value = 0
  logs.value = []
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

// 添加日志
function addLog(topic: string, payload: string, status: 'success' | 'error', message?: string) {
  const now = new Date()
  const time = now.toLocaleTimeString('zh-CN', { hour12: false })
  logs.value.push({ time, topic, payload, status, message })
  
  // 限制日志数量
  if (logs.value.length > 100) {
    logs.value.shift()
  }
  
  // 滚动到底部
  nextTick(() => {
    if (logListRef.value) {
      logListRef.value.scrollTop = logListRef.value.scrollHeight
    }
  })
}

// 开始发布
async function handleStart() {
  if (selectedIds.value.length === 0) {
    ElMessage.warning('请至少选择一条命令')
    return
  }
  
  isRunning.value = true
  currentIndex.value = 0
  currentRound.value = 1
  sentCount.value = 0
  successCount.value = 0
  failCount.value = 0
  logs.value = []
  
  // 通知父组件运行状态变化
  emit('running-change', true)
  
  await publishNext()
}

// 发布下一条
async function publishNext() {
  if (!isRunning.value) return
  
  const commands = getOrderedCommands()
  if (commands.length === 0) {
    stopPublishing()
    return
  }
  
  const command = commands[currentIndex.value]
  currentCommand.value = command
  
  try {
    // 应用发送前处理脚本
    let processedPayload = command.payload
    try {
      const scripts = await invoke<Script[]>('get_enabled_scripts', {
        serverId: props.serverId,
        scriptType: 'before_publish',
      })
      if (scripts.length > 0) {
        processedPayload = await ScriptEngine.executeBeforePublish(scripts, command.payload)
      }
    } catch (e) {
      console.error('脚本处理失败:', e)
    }
    
    await mqttStore.publish(
      props.serverId,
      command.topic,
      processedPayload,
      command.qos,
      command.retain
    )
    successCount.value++
    addLog(command.topic, processedPayload, 'success')
  } catch (error: any) {
    failCount.value++
    addLog(command.topic, command.payload, 'error', error?.message)
  }
  
  sentCount.value++
  currentIndex.value++
  
  // 检查是否完成一轮
  if (currentIndex.value >= commands.length) {
    currentIndex.value = 0
    
    // 检查是否达到循环次数
    if (config.value.loopMode === 'count' && currentRound.value >= config.value.loopCount) {
      stopPublishing(true)
      ElMessage.success('定时发布完成')
      return
    }
    
    currentRound.value++
    
    // 每轮间隔
    if (config.value.roundInterval > 0) {
      publishTimeout = setTimeout(() => publishNext(), config.value.roundInterval)
      return
    }
  }
  
  // 继续下一条
  if (isRunning.value) {
    publishTimeout = setTimeout(() => publishNext(), config.value.interval)
  }
}

// 停止发布
function handleStop() {
  stopPublishing(true)
  ElMessage.info('已停止定时发布')
}

function stopPublishing(keepView: boolean = false) {
  isRunning.value = false
  if (keepView) {
    isCompleted.value = true
  }
  if (publishTimeout) {
    clearTimeout(publishTimeout)
    publishTimeout = null
  }
  // 通知父组件运行状态变化
  emit('running-change', false)
}

// 返回配置界面
function handleBackToConfig() {
  isCompleted.value = false
  currentIndex.value = 0
  currentRound.value = 1
  sentCount.value = 0
  successCount.value = 0
  failCount.value = 0
  logs.value = []
}

// 最小化对话框
function handleMinimize() {
  isMinimized.value = true
  dialogVisible.value = false
  // 通知父组件运行状态
  emit('running-change', true)
}

// 对话框关闭事件（由 el-dialog 的 @close 触发）
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
  emit('running-change', false)
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
