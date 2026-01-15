<template>
  <el-dialog
    v-model="dialogVisible"
    title="脚本管理"
    width="800px"
    :close-on-click-modal="false"
    @open="handleOpen"
  >
    <div class="script-container">
      <!-- 左侧脚本列表 -->
      <div class="script-list-panel">
        <div class="panel-header">
          <span>脚本列表</span>
          <el-button type="primary" size="small" :icon="Plus" @click="handleAdd">
            新增
          </el-button>
        </div>
        <div class="script-list" v-loading="loading">
          <div
            v-for="script in scripts"
            :key="script.id"
            class="script-item"
            :class="{ active: selectedScript?.id === script.id }"
            @click="handleSelect(script)"
          >
            <div class="script-info">
              <span class="script-name">{{ script.name }}</span>
              <el-tag size="small" :type="getTypeTag(script.script_type)">
                {{ getTypeLabel(script.script_type) }}
              </el-tag>
            </div>
            <el-switch
              v-model="script.enabled"
              size="small"
              @change="(val: string | number | boolean) => handleToggle(script, Boolean(val))"
              @click.stop
            />
          </div>
          <el-empty v-if="scripts.length === 0" description="暂无脚本" :image-size="60" />
        </div>
      </div>

      <!-- 右侧编辑区 -->
      <div class="script-editor-panel">
        <template v-if="selectedScript || isAdding">
          <div class="panel-header">
            <span>{{ isAdding ? '新增脚本' : '编辑脚本' }}</span>
            <div class="header-actions">
              <el-button 
                v-if="!isAdding" 
                type="danger" 
                size="small" 
                text 
                :icon="Delete"
                @click="handleDelete"
              >
                删除
              </el-button>
            </div>
          </div>
          <el-form :model="formData" label-width="80px" class="script-form">
            <el-form-item label="名称" required>
              <el-input v-model="formData.name" placeholder="脚本名称" />
            </el-form-item>
            <el-form-item label="类型" required>
              <el-radio-group v-model="formData.script_type" :disabled="!isAdding">
                <el-radio value="before_publish">发送前处理</el-radio>
                <el-radio value="after_receive">接收后处理</el-radio>
              </el-radio-group>
            </el-form-item>
            <el-form-item label="描述">
              <el-input v-model="formData.description" placeholder="脚本描述（可选）" />
            </el-form-item>
            <el-form-item label="脚本代码" required>
              <div class="code-editor-wrapper">
                <div class="code-hint">
                  <code v-if="formData.script_type === 'before_publish'">
                    定义 process(payload) 函数，返回处理后的 payload
                  </code>
                  <code v-else>
                    定义 process(payload, topic) 函数，返回处理后的 payload
                  </code>
                </div>
                <el-input
                  v-model="formData.code"
                  type="textarea"
                  :rows="12"
                  placeholder="请输入 JavaScript 代码"
                  class="code-textarea"
                />
              </div>
            </el-form-item>
          </el-form>
          <div class="form-actions">
            <el-button @click="handleCancel">取消</el-button>
            <el-button type="primary" :loading="saving" @click="handleSave">
              保存
            </el-button>
          </div>
        </template>
        <div v-else class="empty-editor">
          <el-empty description="请选择或新增脚本" :image-size="80" />
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="dialogVisible = false">关闭</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Plus, Delete } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useScriptStore, type Script, type ScriptType } from '@/stores/script'

const props = defineProps<{
  visible: boolean
  serverId: number
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
}>()

const scriptStore = useScriptStore()

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit('update:visible', val)
})

const loading = computed(() => scriptStore.loading)
const scripts = computed(() => scriptStore.scripts)

const selectedScript = ref<Script | null>(null)
const isAdding = ref(false)
const saving = ref(false)

const formData = ref({
  name: '',
  script_type: 'before_publish' as ScriptType,
  code: '',
  description: '',
})

// 打开对话框时加载脚本
function handleOpen() {
  if (props.serverId) {
    scriptStore.loadScripts(props.serverId)
  }
  selectedScript.value = null
  isAdding.value = false
  resetForm()
}

// 重置表单
function resetForm() {
  formData.value = {
    name: '',
    script_type: 'before_publish',
    code: getDefaultCode('before_publish'),
    description: '',
  }
}

// 获取默认代码
function getDefaultCode(type: ScriptType): string {
  if (type === 'before_publish') {
    return `// 发送前处理脚本
// payload: 待发送的消息内容 (字符串)
// 返回值: 处理后的消息内容 (字符串)
// 注意: 必须定义 process 函数，系统会自动调用

function process(payload) {
  // 在这里处理 payload
  // 例如: return payload + '_suffix';
  return payload;
}`
  } else {
    return `// 接收后处理脚本
// payload: 接收到的消息内容 (字符串)
// topic: 消息主题 (字符串)
// 返回值: 处理后的消息内容 (字符串)
// 注意: 必须定义 process 函数，系统会自动调用

function process(payload, topic) {
  // 在这里处理接收到的消息
  // 例如: return JSON.stringify(JSON.parse(payload), null, 2);
  return payload;
}`
  }
}

// 选择脚本
function handleSelect(script: Script) {
  selectedScript.value = script
  isAdding.value = false
  formData.value = {
    name: script.name,
    script_type: script.script_type,
    code: script.code,
    description: script.description || '',
  }
}

// 新增脚本
function handleAdd() {
  selectedScript.value = null
  isAdding.value = true
  resetForm()
}

// 取消编辑
function handleCancel() {
  if (isAdding.value) {
    isAdding.value = false
  } else if (selectedScript.value) {
    handleSelect(selectedScript.value)
  }
}

// 保存脚本
async function handleSave() {
  if (!formData.value.name.trim()) {
    ElMessage.warning('请输入脚本名称')
    return
  }
  if (!formData.value.code.trim()) {
    ElMessage.warning('请输入脚本代码')
    return
  }

  saving.value = true
  try {
    if (isAdding.value) {
      await scriptStore.createScript({
        server_id: props.serverId,
        name: formData.value.name,
        script_type: formData.value.script_type,
        code: formData.value.code,
        enabled: true,
        description: formData.value.description || undefined,
      })
      ElMessage.success('脚本创建成功')
      isAdding.value = false
    } else if (selectedScript.value?.id) {
      await scriptStore.updateScript({
        id: selectedScript.value.id,
        name: formData.value.name,
        code: formData.value.code,
        description: formData.value.description || undefined,
      }, props.serverId)
      ElMessage.success('脚本更新成功')
    }
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`)
  } finally {
    saving.value = false
  }
}

// 删除脚本
async function handleDelete() {
  if (!selectedScript.value?.id) return

  try {
    await ElMessageBox.confirm('确定要删除这个脚本吗？', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })

    await scriptStore.deleteScript(selectedScript.value.id, props.serverId)
    selectedScript.value = null
    ElMessage.success('脚本已删除')
  } catch (e) {
    // 用户取消
  }
}

// 切换脚本状态
async function handleToggle(script: Script, enabled: boolean) {
  if (!script.id) return
  try {
    await scriptStore.toggleScript(script.id, enabled, props.serverId)
  } catch (error) {
    ElMessage.error(`操作失败: ${error}`)
  }
}

// 获取类型标签
function getTypeTag(type: ScriptType) {
  return type === 'before_publish' ? 'primary' : 'success'
}

function getTypeLabel(type: ScriptType) {
  return type === 'before_publish' ? '发送前' : '接收后'
}

// 监听类型变化，更新默认代码
watch(() => formData.value.script_type, (newType) => {
  if (isAdding.value) {
    formData.value.code = getDefaultCode(newType)
  }
})
</script>

<style scoped lang="scss">
.script-container {
  display: flex;
  gap: 16px;
  min-height: 450px;
}

.script-list-panel {
  width: 240px;
  flex-shrink: 0;
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.script-editor-panel {
  flex: 1;
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background-color: var(--card-bg);
  border-bottom: 1px solid var(--app-border-color);
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.script-list {
  flex: 1;
  overflow-y: auto;
}

.script-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--app-border-color);
  cursor: pointer;
  transition: background-color 0.2s;

  &:hover {
    background-color: var(--sidebar-hover);
  }

  &.active {
    background-color: var(--primary-light);
  }

  &:last-child {
    border-bottom: none;
  }
}

.script-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.script-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-color);
}

.script-form {
  padding: 16px;
  flex: 1;
  overflow-y: auto;
}

.code-editor-wrapper {
  width: 100%;
}

.code-hint {
  margin-bottom: 8px;
  padding: 8px 12px;
  background-color: var(--sidebar-hover);
  border-radius: 4px;
  
  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    color: var(--app-text-secondary);
  }
}

.code-textarea {
  :deep(textarea) {
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 13px;
    line-height: 1.5;
  }
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--app-border-color);
}

.empty-editor {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-actions {
  display: flex;
  gap: 8px;
}
</style>
