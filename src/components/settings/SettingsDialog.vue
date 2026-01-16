<template>
  <el-dialog
    v-model="dialogVisible"
    title="系统设置"
    width="500px"
    :close-on-click-modal="false"
    @open="loadSettings"
  >
    <div class="settings-content">
      <!-- 主题设置 -->
      <div class="setting-section">
        <div class="setting-title">外观主题</div>
        <div class="setting-desc">选择应用的外观主题，立即生效</div>
        <el-radio-group 
          v-model="currentTheme" 
          @change="(val: string | number | boolean | undefined) => handleThemeChange(val as Theme)"
          class="theme-radio-group"
        >
          <el-radio-button value="light">
            <el-icon><Sunny /></el-icon>
            浅色
          </el-radio-button>
          <el-radio-button value="dark">
            <el-icon><Moon /></el-icon>
            深色
          </el-radio-button>
          <el-radio-button value="auto">
            <el-icon><Platform /></el-icon>
            跟随系统
          </el-radio-button>
        </el-radio-group>
      </div>

      <!-- 数据存储设置 -->
      <div class="setting-section">
        <div class="setting-title">数据存储</div>
        <div class="setting-desc">配置和脚本等数据的存储位置</div>
        <div class="setting-row">
          <el-tooltip :content="currentDataPath" placement="top">
            <el-input :model-value="currentDataPath" readonly size="small" class="path-input" />
          </el-tooltip>
          <el-tooltip content="更改位置" placement="top">
            <el-button size="small" :icon="FolderOpened" @click="handleSelectFolder" />
          </el-tooltip>
          <el-tooltip content="复制路径" placement="top">
            <el-button size="small" :icon="CopyDocument" @click="handleCopyPath" />
          </el-tooltip>
        </div>
        <el-alert
          v-if="newDataPath"
          type="warning"
          :closable="false"
          show-icon
          class="migrate-alert"
        >
          <template #title>
            <span>将迁移数据到新位置：{{ truncatePath(newDataPath) }}</span>
          </template>
        </el-alert>
      </div>

      <!-- 日志设置 -->
      <div class="setting-section">
        <div class="setting-title">日志</div>
        <div class="setting-desc">错误日志按天分割，最多保留 10 个日志文件</div>
        <div class="setting-row">
          <el-tooltip :content="logPath" placement="top">
            <el-input :model-value="logPath" readonly size="small" class="path-input" />
          </el-tooltip>
          <el-tooltip content="打开日志目录" placement="top">
            <el-button size="small" :icon="FolderOpened" @click="handleOpenLogFolder" />
          </el-tooltip>
          <el-tooltip content="清空日志" placement="top">
            <el-button size="small" :icon="Delete" type="danger" plain @click="handleClearLogs" />
          </el-tooltip>
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="handleClose">取消</el-button>
      <el-button 
        type="primary" 
        :loading="saving"
        :disabled="!hasChanges"
        @click="handleSave"
      >
        保存
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { Sunny, Moon, Platform, FolderOpened, CopyDocument, Delete } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useAppStore, type Theme } from '@/stores/app'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const appStore = useAppStore()

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit('update:visible', val)
})

// 状态
const saving = ref(false)
const currentTheme = ref<Theme>('light')
const originalTheme = ref<Theme>('light')
const currentDataPath = ref('')
const newDataPath = ref('')
const logPath = ref('')

// 是否有更改
const hasChanges = computed(() => {
  return currentTheme.value !== originalTheme.value || newDataPath.value !== ''
})

// 加载设置
async function loadSettings() {
  currentTheme.value = appStore.theme
  originalTheme.value = appStore.theme
  newDataPath.value = ''
  
  try {
    currentDataPath.value = await invoke<string>('get_data_path')
  } catch (e) {
    console.error('获取数据路径失败:', e)
  }
  
  try {
    logPath.value = await invoke<string>('get_log_dir')
  } catch (e) {
    console.error('获取日志路径失败:', e)
  }
}

// 主题变化
function handleThemeChange(theme: Theme) {
  appStore.setTheme(theme)
}

// 选择文件夹
async function handleSelectFolder() {
  try {
    const path = await invoke<string | null>('select_data_folder')
    if (path) {
      newDataPath.value = path
    }
  } catch (e) {
    ElMessage.error(`选择文件夹失败: ${e}`)
  }
}

// 复制路径
async function handleCopyPath() {
  try {
    await navigator.clipboard.writeText(currentDataPath.value)
    ElMessage.success('路径已复制到剪贴板')
  } catch (e) {
    ElMessage.error('复制失败')
  }
}

// 打开日志文件夹
async function handleOpenLogFolder() {
  try {
    await revealItemInDir(logPath.value)
  } catch (e) {
    ElMessage.error(`打开日志目录失败: ${e}`)
  }
}

// 清空日志
async function handleClearLogs() {
  try {
    await ElMessageBox.confirm(
      '确定要清空所有日志文件吗？此操作不可撤销。',
      '清空日志',
      {
        confirmButtonText: '清空',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )
    
    await invoke('clear_logs')
    ElMessage.success('日志已清空')
  } catch (e: any) {
    if (e !== 'cancel') {
      ElMessage.error(`清空日志失败: ${e}`)
    }
  }
}

// 截断路径显示
function truncatePath(path: string): string {
  if (!path) return ''
  const maxLen = 40
  if (path.length <= maxLen) return path
  
  // 显示开头和结尾
  const start = path.substring(0, 15)
  const end = path.substring(path.length - 20)
  return `${start}...${end}`
}

// 保存设置
async function handleSave() {
  saving.value = true
  
  try {
    // 如果有新的数据路径
    if (newDataPath.value) {
      const action = await ElMessageBox.confirm(
        '是否将现有数据迁移到新位置？',
        '更改数据存储位置',
        {
          confirmButtonText: '迁移数据',
          cancelButtonText: '仅更改路径',
          distinguishCancelAndClose: true,
          type: 'info',
        }
      ).then(() => 'migrate').catch((action: string) => action)
      
      if (action === 'close') {
        saving.value = false
        return
      }
      
      const shouldMigrate = action === 'migrate'
      await invoke('migrate_data_path', { newPath: newDataPath.value, migrate: shouldMigrate })
      
      if (shouldMigrate) {
        ElMessage.success('数据迁移完成，请重启应用')
      } else {
        ElMessage.success('存储位置已更改，请重启应用')
      }
    }
    
    dialogVisible.value = false
  } catch (e: any) {
    if (e !== 'cancel') {
      ElMessage.error(`保存设置失败: ${e}`)
    }
  } finally {
    saving.value = false
  }
}

// 关闭对话框
function handleClose() {
  // 如果主题已更改但未保存，恢复原始主题
  if (currentTheme.value !== originalTheme.value && !saving.value) {
    appStore.setTheme(originalTheme.value)
  }
  dialogVisible.value = false
}
</script>

<style scoped lang="scss">
.settings-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.setting-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.setting-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.setting-desc {
  font-size: 12px;
  color: var(--app-text-secondary);
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.path-input {
  flex: 1;
  
  :deep(input) {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
  }
}

.migrate-alert {
  margin-top: 4px;
}

.theme-radio-group {
  :deep(.el-radio-button__inner) {
    display: flex;
    align-items: center;
    gap: 4px;
  }
}
</style>
