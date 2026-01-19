<template>
  <div class="env-drawer">
    <!-- 工具栏 -->
    <div class="drawer-toolbar">
      <el-input
        v-model="searchKeyword"
        :placeholder="$t('env.searchPlaceholder')"
        :prefix-icon="Search"
        clearable
        size="default"
        @input="handleSearch"
      />
      <el-button type="primary" :icon="Plus" @click="handleCreate">
        {{ $t('env.addVariable') }}
      </el-button>
    </div>

    <!-- 变量卡片列表 -->
    <div class="env-cards" v-loading="loading">
      <div
        v-for="variable in filteredVariables"
        :key="variable.id"
        class="env-card"
      >
        <div class="card-header">
          <span class="card-name">{{ variable.name }}</span>
          <el-dropdown trigger="click" @command="(cmd: string) => handleCommand(cmd, variable)">
            <el-button :icon="MoreFilled" text size="small" class="more-btn" />
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="edit" :icon="Edit">{{ $t('common.edit') }}</el-dropdown-item>
                <el-dropdown-item command="copy" :icon="CopyDocument">{{ $t('env.copyValue') }}</el-dropdown-item>
                <el-dropdown-item command="delete" :icon="Delete" divided>
                  <span style="color: var(--el-color-danger)">{{ $t('common.delete') }}</span>
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>

        <div class="card-value">
          <div class="value-content" :class="{ masked: !showValues[variable.id!] }">
            {{ variable.value }}
          </div>
          <!-- <el-button
            :icon="showValues[variable.id!] ? Hide : View"
            text
            size="small"
            class="toggle-btn"
            @click="toggleShowValue(variable.id!)"
          /> -->
        </div>

        <div v-if="variable.description" class="card-desc">
          {{ variable.description }}
        </div>

        <div class="card-usage">
          <code v-text="getUsageSyntax(variable.name)"></code>
          <el-button
            :icon="CopyDocument"
            text
            size="small"
            @click="copyUsage(variable.name)"
          />
        </div>
      </div>

      <!-- 空状态 -->
      <el-empty
        v-if="!loading && filteredVariables.length === 0"
        :description="$t('env.noVariables')"
        :image-size="80"
      >
        <el-button type="primary" @click="handleCreate">
          {{ $t('env.addVariable') }}
        </el-button>
      </el-empty>
    </div>

    <!-- 创建/编辑对话框 -->
    <EnvDialog
      v-model:visible="showDialog"
      :variable="editingVariable"
      :server-id="serverId"
      @saved="handleSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, reactive } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Search,
  Plus,
  MoreFilled,
  Edit,
  CopyDocument,
  Delete,
  View,
  Hide
} from '@element-plus/icons-vue'
import { useEnvStore, type EnvVariable } from '@/stores/env'
import EnvDialog from './EnvDialog.vue'

const { t } = useI18n()

const props = defineProps<{
  serverId: number
}>()

const envStore = useEnvStore()

const searchKeyword = ref('')
const showDialog = ref(false)
const editingVariable = ref<EnvVariable | null>(null)
const showValues = reactive<Record<number, boolean>>({})

const loading = computed(() => envStore.loading)
const filteredVariables = computed(() => envStore.filteredVariables)

// 初始化加载
onMounted(() => {
  if (props.serverId) {
    envStore.loadVariables(props.serverId)
  }
})

// 监听serverId变化
watch(() => props.serverId, (newId) => {
  if (newId) {
    envStore.loadVariables(newId)
  }
})

// 遮蔽值显示
function maskValue(value: string): string {
  if (value.length <= 4) {
    return '*'.repeat(value.length)
  }
  return value.substring(0, 2) + '*'.repeat(Math.min(value.length - 4, 8)) + value.substring(value.length - 2)
}

// 获取使用语法
function getUsageSyntax(name: string): string {
  return '{{' + name + '}}'
}

// 切换显示/隐藏值
function toggleShowValue(id: number) {
  showValues[id] = !showValues[id]
}

// 搜索处理
function handleSearch() {
  envStore.setSearchKeyword(searchKeyword.value)
}

// 创建变量
function handleCreate() {
  editingVariable.value = null
  showDialog.value = true
}

// 复制使用语法
async function copyUsage(name: string) {
  try {
    await navigator.clipboard.writeText(getUsageSyntax(name))
    ElMessage.success(t('success.copied'))
  } catch (error) {
    ElMessage.error(t('errors.copyFailed'))
  }
}

// 复制值
async function copyValue(variable: EnvVariable) {
  try {
    await navigator.clipboard.writeText(variable.value)
    ElMessage.success(t('success.copied'))
  } catch (error) {
    ElMessage.error(t('errors.copyFailed'))
  }
}

// 下拉菜单命令
async function handleCommand(command: string, variable: EnvVariable) {
  switch (command) {
    case 'edit':
      editingVariable.value = variable
      showDialog.value = true
      break
    case 'copy':
      await copyValue(variable)
      break
    case 'delete':
      try {
        await ElMessageBox.confirm(
          t('env.deleteConfirm', { name: variable.name }),
          t('env.deleteTitle'),
          { type: 'warning', confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel') }
        )
        await envStore.deleteVariable(variable.id!)
        ElMessage.success(t('env.deleteSuccess'))
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(t('errors.deleteFailed'))
        }
      }
      break
  }
}

// 保存成功回调
function handleSaved() {
  showDialog.value = false
  editingVariable.value = null
  ElMessage.success(t('env.saveSuccess'))
}
</script>

<style scoped lang="scss">
.env-drawer {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 0 4px;
}

.drawer-toolbar {
  display: flex;
  gap: 12px;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.drawer-toolbar .el-input {
  flex: 1;
}

.env-cards {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-right: 4px;
}

.env-card {
  background-color: var(--card-bg);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  padding: 14px;
  transition: all 0.2s ease;

  &:hover {
    border-color: var(--primary-color);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.card-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--primary-color);
  font-family: 'Fira Code', 'Consolas', monospace;
}

.more-btn {
  opacity: 0.6;
  transition: opacity 0.2s;

  &:hover {
    opacity: 1;
  }
}

.card-value {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  padding: 8px 12px;
  background-color: var(--sidebar-hover);
  border-radius: 6px;
}

.value-content {
  flex: 1;
  font-family: 'Fira Code', 'Consolas', monospace;
  font-size: 13px;
  color: var(--app-text-color);
  word-break: break-all;

  &.masked {
    color: var(--app-text-secondary);
  }
}

.toggle-btn {
  flex-shrink: 0;
  opacity: 0.6;

  &:hover {
    opacity: 1;
  }
}

.card-desc {
  font-size: 12px;
  color: var(--app-text-secondary);
  margin-bottom: 10px;
  line-height: 1.5;
}

.card-usage {
  display: flex;
  align-items: center;
  gap: 4px;

  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    color: var(--app-text-secondary);
    background-color: var(--sidebar-hover);
    padding: 2px 8px;
    border-radius: 4px;
  }
}
</style>
