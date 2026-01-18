<template>
  <el-dialog
    :model-value="visible"
    :title="isEdit ? $t('env.editVariable') : $t('env.addVariable')"
    width="480px"
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="$emit('update:visible', $event)"
  >
    <el-form
      ref="formRef"
      :model="form"
      :rules="rules"
      label-width="80px"
      label-position="right"
    >
      <el-form-item :label="$t('env.name')" prop="name">
        <el-input
          v-model="form.name"
          :placeholder="$t('env.namePlaceholder')"
          maxlength="50"
          show-word-limit
        />
        <div class="form-hint">{{ $t('env.nameHint') }}</div>
      </el-form-item>

      <el-form-item :label="$t('env.value')" prop="value">
        <el-input
          v-model="form.value"
          :placeholder="$t('env.valuePlaceholder')"
          type="textarea"
          :rows="3"
        />
      </el-form-item>

      <el-form-item :label="$t('env.description')" prop="description">
        <el-input
          v-model="form.description"
          :placeholder="$t('env.descriptionPlaceholder')"
          type="textarea"
          :rows="2"
          maxlength="200"
        />
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="dialog-footer">
        <el-button @click="$emit('update:visible', false)">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSubmit" :loading="submitting">
          {{ $t('common.save') }}
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
import { useEnvStore, type EnvVariable } from '@/stores/env'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  variable: EnvVariable | null
  serverId: number
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  saved: []
}>()

const envStore = useEnvStore()
const formRef = ref<FormInstance>()
const submitting = ref(false)

const isEdit = computed(() => !!props.variable?.id)

// 表单数据
const form = ref({
  name: '',
  value: '',
  description: ''
})

// 变量名校验函数
const validateName = (_rule: any, value: string, callback: (error?: Error) => void) => {
  if (!value) {
    callback(new Error(t('errors.inputName')))
    return
  }
  // 以字母开头，只能包含字母、数字、下划线
  if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(value)) {
    callback(new Error(t('env.invalidName')))
    return
  }
  callback()
}

// 表单验证规则
const rules: FormRules = {
  name: [
    { required: true, validator: validateName, trigger: 'blur' }
  ],
  value: [
    { required: true, message: t('errors.inputValue'), trigger: 'blur' }
  ]
}

// 监听visible变化，初始化或重置表单
watch(() => props.visible, (visible) => {
  if (visible) {
    if (props.variable) {
      // 编辑模式：填充数据
      form.value = {
        name: props.variable.name,
        value: props.variable.value,
        description: props.variable.description || ''
      }
    } else {
      // 新建模式：重置表单
      resetForm()
    }
  }
})

// 重置表单
function resetForm() {
  form.value = {
    name: '',
    value: '',
    description: ''
  }
  formRef.value?.resetFields()
}

// 提交表单
async function handleSubmit() {
  if (!formRef.value) return

  const valid = await formRef.value.validate().catch(() => false)
  if (!valid) return

  submitting.value = true
  try {
    if (isEdit.value) {
      await envStore.updateVariable({
        id: props.variable!.id!,
        name: form.value.name,
        value: form.value.value,
        description: form.value.description || undefined
      })
    } else {
      await envStore.createVariable({
        server_id: props.serverId,
        name: form.value.name,
        value: form.value.value,
        description: form.value.description || undefined
      })
    }
    emit('saved')
  } catch (error: any) {
    if (error?.toString().includes('already exists')) {
      ElMessage.error(t('env.duplicateName'))
    } else {
      ElMessage.error(`${t('errors.saveFailed')}: ${error}`)
    }
  } finally {
    submitting.value = false
  }
}
</script>

<style scoped lang="scss">
.form-hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--app-text-secondary);
  line-height: 1.4;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}
</style>
