<template>
  <el-dialog
    v-model="dialogVisible"
    :title="$t('script.title')"
    width="800px"
    :close-on-click-modal="false"
    @open="handleOpen"
  >
    <div class="script-container">
      <!-- 左侧脚本列表 -->
      <div class="script-list-panel">
        <div class="panel-header">
          <span>{{ $t('script.title') }}</span>
          <el-button type="primary" size="small" :icon="Plus" @click="handleAdd">
            {{ $t('script.addScript') }}
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
              <el-tag size="small" style="width: 100px;" :type="getTypeTag(script.script_type)">
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
          <el-empty v-if="scripts.length === 0" :description="$t('script.noScript')" :image-size="60" />
        </div>
      </div>

      <!-- 右侧编辑区 -->
      <div class="script-editor-panel">
        <template v-if="selectedScript || isAdding">
          <div class="panel-header">
            <span>{{ isAdding ? $t('script.addScript') : $t('script.editScript') }}</span>
            <div class="header-actions">
              <el-button 
                v-if="!isAdding" 
                type="danger" 
                size="small" 
                text 
                :icon="Delete"
                @click="handleDelete"
              >
                {{ $t('common.delete') }}
              </el-button>
            </div>
          </div>
          <el-form :model="formData" label-width="100px" class="script-form">
            <el-form-item :label="$t('script.name')" required>
              <el-input v-model="formData.name" :placeholder="$t('script.namePlaceholder')" />
            </el-form-item>
            <el-form-item :label="$t('script.type')" required>
              <el-radio-group v-model="formData.script_type" :disabled="!isAdding">
                <el-radio value="before_publish">{{ $t('script.beforeSend') }}</el-radio>
                <el-radio value="after_receive">{{ $t('script.afterReceive') }}</el-radio>
              </el-radio-group>
            </el-form-item>
            <el-form-item :label="$t('script.description')">
              <el-input v-model="formData.description" :placeholder="$t('script.descriptionPlaceholder')" />
            </el-form-item>
            <el-form-item :label="$t('script.code')" required>
              <div class="code-editor-wrapper">
                <div class="code-hint">
                  <code v-if="formData.script_type === 'before_publish'">
                    Define process(payload) function, return processed payload
                  </code>
                  <code v-else>
                    Define process(payload, topic) function, return processed payload
                  </code>
                </div>
                <el-input
                  v-model="formData.code"
                  type="textarea"
                  :rows="12"
                  placeholder="Enter JavaScript code"
                  class="code-textarea"
                />
              </div>
            </el-form-item>
          </el-form>
          <div class="form-actions">
            <div class="left-actions">
              <el-button :icon="FolderOpened" @click="handleImportFile">Import JS</el-button>
              <el-button :icon="Document" @click="showFunctionList = true">Functions</el-button>
            </div>
            <el-button type="primary" :loading="saving" @click="handleSave">
              {{ $t('common.save') }}
            </el-button>
          </div>
        </template>
        <div v-else class="empty-editor">
          <el-empty :description="$t('script.noScript')" :image-size="80" />
        </div>
      </div>
    </div>

    <template #footer>
      <el-button @click="dialogVisible = false">{{ $t('common.close') }}</el-button>
    </template>
  </el-dialog>

  <!-- 可用函数列表弹框 -->
  <el-dialog
    v-model="showFunctionList"
    :title="$t('script.functions')"
    width="600px"
    :append-to-body="true"
  >
    <div class="function-list">
      <div class="function-category">
        <h4>Basic Functions</h4>
        <el-table :data="basicFunctions" size="small">
          <el-table-column prop="name" label="Function" width="200" />
          <el-table-column prop="desc" label="Description" />
        </el-table>
      </div>
      
      <div class="function-category">
        <h4>Encoding</h4>
        <el-table :data="encodingFunctions" size="small">
          <el-table-column prop="name" label="Function" width="250" />
          <el-table-column prop="desc" label="Description" />
        </el-table>
      </div>
      
      <div class="function-category">
        <h4>Hash Functions</h4>
        <el-table :data="hashFunctions" size="small">
          <el-table-column prop="name" label="Function" width="250" />
          <el-table-column prop="desc" label="Description" />
        </el-table>
      </div>
      
      <div class="function-category">
        <h4>AES Encryption</h4>
        <el-table :data="aesFunctions" size="small">
          <el-table-column prop="name" label="Function" width="320" />
          <el-table-column prop="desc" label="Description" />
        </el-table>
      </div>
      
      <div class="function-category">
        <h4>Other Tools</h4>
        <el-table :data="otherFunctions" size="small">
          <el-table-column prop="name" label="Function" width="200" />
          <el-table-column prop="desc" label="Description" />
        </el-table>
      </div>
    </div>

    <template #footer>
      <el-button @click="showFunctionList = false">{{ $t('common.close') }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Delete, Document, FolderOpened } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useScriptStore, type Script, type ScriptType } from '@/stores/script'
import { open } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'

const { t } = useI18n()

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
const showFunctionList = ref(false)

// 可用函数列表数据
const basicFunctions = [
  { name: 'JSON.parse(str)', desc: 'Parse JSON string' },
  { name: 'JSON.stringify(obj)', desc: 'Object to JSON string' },
  { name: 'console.log(...args)', desc: 'Output log (debug)' },
  { name: 'btoa(str)', desc: 'Base64 encode' },
  { name: 'atob(str)', desc: 'Base64 decode' },
]

const encodingFunctions = [
  { name: 'crypto.stringToBytes(str)', desc: 'String to Uint8Array' },
  { name: 'crypto.bytesToString(bytes)', desc: 'Uint8Array to string' },
  { name: 'crypto.bytesToBase64(bytes)', desc: 'Uint8Array to Base64' },
  { name: 'crypto.base64ToBytes(str)', desc: 'Base64 to Uint8Array' },
  { name: 'crypto.bytesToHex(bytes)', desc: 'Uint8Array to hex' },
  { name: 'crypto.hexToBytes(hex)', desc: 'Hex to Uint8Array' },
]

const hashFunctions = [
  { name: 'await crypto.sha256(data)', desc: 'SHA-256 hash (returns hex)' },
  { name: 'await crypto.sha1(data)', desc: 'SHA-1 hash (returns hex)' },
  { name: 'crypto.md5(data)', desc: 'MD5 hash (returns hex)' },
  { name: 'await crypto.hmacSha256(key, data)', desc: 'HMAC-SHA256' },
]

const aesFunctions = [
  { name: 'await crypto.aesGcmEncrypt(text, keyBase64, ivBase64?)', desc: 'AES-GCM encrypt (Base64)' },
  { name: 'await crypto.aesGcmDecrypt(cipherBase64, keyBase64)', desc: 'AES-GCM decrypt (Base64)' },
  { name: 'await crypto.aesCbcEncrypt(text, keyBase64, ivBase64?)', desc: 'AES-CBC encrypt (Base64)' },
  { name: 'await crypto.aesCbcDecrypt(cipherBase64, keyBase64)', desc: 'AES-CBC decrypt (Base64)' },
  { name: 'await crypto.aesGcmEncryptHex(text, keyHex, ivHex?)', desc: 'AES-GCM encrypt (Hex)' },
  { name: 'await crypto.aesGcmDecryptHex(cipherHex, keyHex)', desc: 'AES-GCM decrypt (Hex)' },
  { name: 'await crypto.aesCbcEncryptHex(text, keyHex, ivHex?)', desc: 'AES-CBC encrypt (Hex)' },
  { name: 'await crypto.aesCbcDecryptHex(cipherHex, keyHex)', desc: 'AES-CBC decrypt (Hex)' },
]

const otherFunctions = [
  { name: 'crypto.generateKey(bits)', desc: 'Generate random key (Base64)' },
  { name: 'crypto.generateIv(length)', desc: 'Generate random IV (Base64)' },
  { name: 'crypto.randomBytes(length)', desc: 'Generate random bytes' },
  { name: 'crypto.xor(data, key)', desc: 'XOR encrypt/decrypt' },
  { name: 'crypto.crc32(data)', desc: 'CRC32 checksum' },
]

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
    return `// Define process function to handle payload. Click "Functions" to see available APIs

async function process(payload) {
  // do nothing
  return payload;
}`
  } else {
    return `// Define process function to handle payload. Click "Functions" to see available APIs

async function process(payload, topic) {
  // do nothing
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

// 保存脚本
async function handleSave() {
  if (!formData.value.name.trim()) {
    ElMessage.warning(t('errors.inputName'))
    return
  }
  if (!formData.value.code.trim()) {
    ElMessage.warning(t('script.code'))
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
      ElMessage.success(t('script.saveSuccess'))
      isAdding.value = false
    } else if (selectedScript.value?.id) {
      await scriptStore.updateScript({
        id: selectedScript.value.id,
        name: formData.value.name,
        code: formData.value.code,
        description: formData.value.description || undefined,
      }, props.serverId)
      ElMessage.success(t('script.saveSuccess'))
    }
  } catch (error) {
    ElMessage.error(`${t('errors.saveFailed')}: ${error}`)
  } finally {
    saving.value = false
  }
}

// 删除脚本
async function handleDelete() {
  if (!selectedScript.value?.id) return

  try {
    await ElMessageBox.confirm(t('script.deleteConfirm'), t('script.deleteTitle'), {
      confirmButtonText: t('common.delete'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
    })

    await scriptStore.deleteScript(selectedScript.value.id, props.serverId)
    selectedScript.value = null
    ElMessage.success(t('script.deleteSuccess'))
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
    ElMessage.error(`${t('errors.saveFailed')}: ${error}`)
  }
}

// 获取类型标签
function getTypeTag(type: ScriptType) {
  return type === 'before_publish' ? 'primary' : 'success'
}

function getTypeLabel(type: ScriptType) {
  return type === 'before_publish' ? t('script.beforeSend') : t('script.afterReceive')
}

// 监听类型变化，更新默认代码
watch(() => formData.value.script_type, (newType) => {
  if (isAdding.value) {
    formData.value.code = getDefaultCode(newType)
  }
})

// 导入文件
async function handleImportFile() {
  try {
    const filePath = await open({
      multiple: false,
      filters: [{
        name: 'Script Files',
        extensions: ['js', 'txt']
      }]
    })
    
    if (filePath) {
      const content = await readTextFile(filePath as string)
      formData.value.code = content
      ElMessage.success(t('success.saved'))
    }
  } catch (error) {
    ElMessage.error(`${t('errors.loadFailed')}: ${error}`)
  }
}
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

.script-info :deep(.el-tag) {
  width: 56px;
  text-align: center;
  justify-content: center;
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
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-top: 1px solid var(--app-border-color);
}

.left-actions {
  display: flex;
  gap: 8px;
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

.function-list {
  max-height: 500px;
  overflow-y: auto;
}

.function-category {
  margin-bottom: 20px;
  
  &:last-child {
    margin-bottom: 0;
  }
  
  h4 {
    margin: 0 0 8px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--app-text-color);
    border-left: 3px solid var(--el-color-primary);
    padding-left: 8px;
  }
}
</style>
