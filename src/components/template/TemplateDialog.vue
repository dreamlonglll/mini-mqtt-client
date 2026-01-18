<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? $t('script.editScript') : $t('template.addTemplate')"
    width="640px"
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="$emit('update:visible', $event)"
  >
    <el-form
      ref="formRef"
      :model="form"
      :rules="rules"
      label-width="100px"
      label-position="right"
    >
      <!-- 基本信息 -->
      <div class="form-section">
        <div class="section-title">{{ $t('template.name') }}</div>

        <el-form-item :label="$t('template.name')" prop="name">
          <el-input
            v-model="form.name"
            :placeholder="$t('template.namePlaceholder')"
            maxlength="50"
            show-word-limit
          />
        </el-form-item>

        <el-form-item :label="$t('template.category')" prop="category">
          <el-select
            v-model="form.category"
            :placeholder="$t('template.categoryPlaceholder')"
            filterable
            allow-create
            default-first-option
            clearable
            class="full-width"
          >
            <el-option
              v-for="cat in categories"
              :key="cat"
              :label="cat"
              :value="cat"
            />
          </el-select>
        </el-form-item>

        <el-form-item :label="$t('script.description')" prop="description">
          <el-input
            v-model="form.description"
            type="textarea"
            :rows="2"
            :placeholder="$t('script.descriptionPlaceholder')"
            maxlength="200"
          />
        </el-form-item>
      </div>

      <!-- MQTT 配置 -->
      <div class="form-section">
        <div class="section-title">MQTT</div>

        <el-form-item :label="$t('publish.topic')" prop="topic">
          <el-input
            v-model="form.topic"
            :placeholder="$t('publish.topicPlaceholder')"
          >
            <template #prefix>
              <el-icon><Position /></el-icon>
            </template>
          </el-input>
        </el-form-item>

        <el-row :gutter="16">
          <el-col :span="12">
            <el-form-item label="QoS" prop="qos">
              <el-radio-group v-model="form.qos" class="qos-group">
                <el-radio-button :value="0">QoS 0</el-radio-button>
                <el-radio-button :value="1">QoS 1</el-radio-button>
                <el-radio-button :value="2">QoS 2</el-radio-button>
              </el-radio-group>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="Retain" prop="retain">
              <el-switch v-model="form.retain" />
              <span class="form-hint">{{ $t('publish.retain') }}</span>
            </el-form-item>
          </el-col>
        </el-row>
      </div>

      <!-- 消息内容 -->
      <div class="form-section">
        <div class="section-title">
          <span>{{ $t('publish.payload') }}</span>
          <el-radio-group v-model="form.payload_type" size="small" class="type-selector">
            <el-radio-button value="json">JSON</el-radio-button>
            <el-radio-button value="text">TEXT</el-radio-button>
            <el-radio-button value="hex">HEX</el-radio-button>
          </el-radio-group>
        </div>

        <el-form-item prop="payload" label-width="0">
          <div class="payload-editor-wrapper">
            <el-input
              v-model="form.payload"
              type="textarea"
              :rows="8"
              :placeholder="payloadPlaceholder"
              class="payload-editor"
              :class="{ 'has-error': !!payloadError }"
            />
            <div class="editor-toolbar">
              <el-button
                v-if="form.payload_type === 'json'"
                size="small"
                text
                @click="formatJson"
              >
                <el-icon><MagicStick /></el-icon>
                {{ $t('template.format') }}
              </el-button>
              <el-button
                v-if="form.payload_type === 'json'"
                size="small"
                text
                @click="compressJson"
              >
                <el-icon><Minus /></el-icon>
                {{ $t('template.minify') }}
              </el-button>
              <span v-if="payloadError" class="error-text">
                <el-icon><WarningFilled /></el-icon>
                {{ payloadError }}
              </span>
            </div>
          </div>
        </el-form-item>
      </div>
    </el-form>

    <template #footer>
      <div class="dialog-footer">
        <el-button @click="$emit('update:visible', false)">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSubmit" :loading="submitting">
          {{ isEdit ? $t('common.save') : $t('template.addTemplate') }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { FormInstance, FormRules } from 'element-plus'
import { ElMessage } from 'element-plus'
import {
  Position,
  MagicStick,
  Minus,
  WarningFilled
} from '@element-plus/icons-vue'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  template: CommandTemplate | null
  serverId: number
  categories: string[]
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  saved: []
}>()

const templateStore = useTemplateStore()
const formRef = ref<FormInstance>()
const submitting = ref(false)
const payloadError = ref('')

const isEdit = computed(() => !!props.template?.id)

// 表单数据
const form = ref({
  name: '',
  category: '',
  description: '',
  topic: '',
  payload_type: 'json' as 'json' | 'text' | 'hex',
  payload: '',
  qos: 0 as 0 | 1 | 2,
  retain: false
})

// 表单验证规则
const rules: FormRules = {
  name: [
    { required: true, message: t('errors.inputName'), trigger: 'blur' },
    { min: 1, max: 50, message: 'Name must be 1-50 characters', trigger: 'blur' }
  ],
  topic: [
    { required: true, message: t('errors.inputTopic'), trigger: 'blur' }
  ],
  payload: [
    { required: true, message: t('errors.inputPayload'), trigger: 'blur' }
  ]
}

// 占位符文本
const payloadPlaceholder = computed(() => {
  switch (form.value.payload_type) {
    case 'json':
      return '{\n  "action": "start",\n  "value": 100\n}'
    case 'hex':
      return '48 65 6C 6C 6F 20 57 6F 72 6C 64'
    default:
      return t('publish.payloadPlaceholder')
  }
})

// 监听visible变化，初始化或重置表单
watch(() => props.visible, (visible) => {
  if (visible) {
    payloadError.value = ''
    if (props.template) {
      // 编辑模式：填充数据
      form.value = {
        name: props.template.name,
        category: props.template.category || '',
        description: props.template.description || '',
        topic: props.template.topic,
        payload_type: props.template.payload_type,
        payload: props.template.payload,
        qos: props.template.qos,
        retain: props.template.retain
      }
    } else {
      // 新建模式：重置表单
      resetForm()
    }
  }
})

// 监听payload_type变化，验证内容格式
watch(() => form.value.payload_type, () => {
  validatePayload()
})

// 监听payload变化
watch(() => form.value.payload, () => {
  if (form.value.payload_type === 'json') {
    validatePayload()
  } else {
    payloadError.value = ''
  }
})

// 重置表单
function resetForm() {
  form.value = {
    name: '',
    category: '',
    description: '',
    topic: '',
    payload_type: 'json',
    payload: '',
    qos: 0,
    retain: false
  }
  formRef.value?.resetFields()
}

// 验证payload格式
function validatePayload(): boolean {
  if (form.value.payload_type === 'json' && form.value.payload.trim()) {
    try {
      JSON.parse(form.value.payload)
      payloadError.value = ''
      return true
    } catch (e) {
      payloadError.value = t('errors.jsonInvalid')
      return false
    }
  } else if (form.value.payload_type === 'hex' && form.value.payload.trim()) {
    const hex = form.value.payload.replace(/\s/g, '')
    if (!/^[0-9A-Fa-f]*$/.test(hex)) {
      payloadError.value = t('errors.hexInvalid')
      return false
    }
    payloadError.value = ''
    return true
  }
  payloadError.value = ''
  return true
}

// 格式化JSON
function formatJson() {
  try {
    const parsed = JSON.parse(form.value.payload)
    form.value.payload = JSON.stringify(parsed, null, 2)
    payloadError.value = ''
  } catch (e) {
    payloadError.value = t('errors.jsonInvalid')
  }
}

// 压缩JSON
function compressJson() {
  try {
    const parsed = JSON.parse(form.value.payload)
    form.value.payload = JSON.stringify(parsed)
    payloadError.value = ''
  } catch (e) {
    payloadError.value = t('errors.jsonInvalid')
  }
}

// 提交表单
async function handleSubmit() {
  if (!formRef.value) return

  const valid = await formRef.value.validate().catch(() => false)
  if (!valid) return

  // 验证payload格式
  if (!validatePayload()) {
    return
  }

  submitting.value = true
  try {
    if (isEdit.value) {
      await templateStore.updateTemplate({
        id: props.template!.id!,
        name: form.value.name,
        category: form.value.category || undefined,
        topic: form.value.topic,
        payload_type: form.value.payload_type,
        payload: form.value.payload,
        qos: form.value.qos,
        retain: form.value.retain,
        description: form.value.description || undefined
      })
    } else {
      await templateStore.createTemplate({
        server_id: props.serverId,
        name: form.value.name,
        topic: form.value.topic,
        payload: form.value.payload,
        payload_type: form.value.payload_type,
        qos: form.value.qos,
        retain: form.value.retain,
        category: form.value.category || undefined,
        description: form.value.description || undefined
      })
    }
    emit('saved')
  } catch (error) {
    ElMessage.error(`${t('errors.saveFailed')}: ${error}`)
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped lang="scss">
.form-section {
  margin-bottom: 24px;

  &:last-child {
    margin-bottom: 0;
  }
}

.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--app-border-color);
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

.full-width {
  width: 100%;
}

.qos-group {
  display: flex;
  flex-wrap: nowrap;

  :deep(.el-radio-button) {
    flex: 1;
  }

  :deep(.el-radio-button__inner) {
    width: 100%;
    white-space: nowrap;
  }
}

.form-hint {
  margin-left: 8px;
  font-size: 12px;
  color: var(--app-text-secondary);
}

.type-selector {
  :deep(.el-radio-button__inner) {
    padding: 4px 12px;
  }
}

.payload-editor-wrapper {
  width: 100%;
}

.payload-editor {
  font-family: 'Fira Code', 'Consolas', monospace;

  &.has-error {
    :deep(.el-textarea__inner) {
      border-color: var(--el-color-danger);
    }
  }

  :deep(.el-textarea__inner) {
    font-family: inherit;
    font-size: 13px;
    line-height: 1.6;
  }
}

.editor-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  min-height: 28px;
}

.error-text {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
  font-size: 12px;
  color: var(--el-color-danger);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
</style>
