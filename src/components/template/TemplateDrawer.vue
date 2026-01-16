<template>
  <div class="template-drawer">
    <!-- 工具栏 -->
    <div class="drawer-toolbar">
      <el-input
        v-model="searchKeyword"
        :placeholder="$t('template.searchPlaceholder')"
        :prefix-icon="Search"
        clearable
        size="default"
        @input="handleSearch"
      />
      <el-button type="primary" :icon="Plus" @click="handleCreate">
        {{ $t('template.addTemplate') }}
      </el-button>
    </div>

    <!-- 分类筛选 -->
    <div v-if="categories.length > 0" class="category-filter">
      <el-tag
        :type="!selectedCategory ? 'primary' : 'info'"
        class="category-tag"
        effect="plain"
        @click="selectedCategory = null"
      >
        {{ $t('template.allCategories') }}
      </el-tag>
      <el-tag
        v-for="cat in categories"
        :key="cat"
        :type="selectedCategory === cat ? 'primary' : 'info'"
        class="category-tag"
        effect="plain"
        @click="selectedCategory = cat"
      >
        {{ cat }}
      </el-tag>
    </div>

    <!-- 模板卡片列表 -->
    <div class="template-cards" v-loading="loading">
      <div
        v-for="template in filteredTemplates"
        :key="template.id"
        class="template-card"
      >
        <div class="card-header">
          <span class="card-name">{{ template.name }}</span>
          <el-dropdown trigger="click" @command="(cmd: string) => handleCommand(cmd, template)">
            <el-button :icon="MoreFilled" text size="small" class="more-btn" />
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="edit" :icon="Edit">{{ $t('common.edit') }}</el-dropdown-item>
                <el-dropdown-item command="duplicate" :icon="CopyDocument">{{ $t('common.copy') }}</el-dropdown-item>
                <el-dropdown-item command="delete" :icon="Delete" divided>
                  <span style="color: var(--el-color-danger)">{{ $t('common.delete') }}</span>
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>

        <div class="card-topic">
          <el-icon><Position /></el-icon>
          <code>{{ template.topic }}</code>
        </div>

        <div class="card-payload">
          <pre>{{ formatPayload(template.payload) }}</pre>
        </div>

        <div class="card-footer">
          <div class="card-meta">
            <el-tag size="small" :type="getPayloadTypeTag(template.payload_type)">
              {{ template.payload_type.toUpperCase() }}
            </el-tag>
            <span class="meta-item">QoS {{ template.qos }}</span>
            <span v-if="template.retain" class="meta-item retain">Retain</span>
          </div>
          <el-button type="primary" size="small" @click="handleUse(template)">
            {{ $t('publish.send') }}
          </el-button>
        </div>
      </div>

      <!-- 空状态 -->
      <el-empty
        v-if="!loading && filteredTemplates.length === 0"
        :description="$t('template.noTemplate')"
        :image-size="80"
      >
        <el-button type="primary" @click="handleCreate">
          {{ $t('template.addTemplate') }}
        </el-button>
      </el-empty>
    </div>

    <!-- 创建/编辑对话框 -->
    <TemplateDialog
      v-model:visible="showDialog"
      :template="editingTemplate"
      :server-id="serverId"
      :categories="categories"
      @saved="handleSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Search,
  Plus,
  MoreFilled,
  Position,
  Edit,
  CopyDocument,
  Delete
} from '@element-plus/icons-vue'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'
import TemplateDialog from './TemplateDialog.vue'

const { t } = useI18n()

const props = defineProps<{
  serverId: number
}>()

const emit = defineEmits<{
  use: [template: CommandTemplate]
}>()

const templateStore = useTemplateStore()

const searchKeyword = ref('')
const selectedCategory = ref<string | null>(null)
const showDialog = ref(false)
const editingTemplate = ref<CommandTemplate | null>(null)

const loading = computed(() => templateStore.loading)
const categories = computed(() => templateStore.categories)

// 过滤模板
const filteredTemplates = computed(() => {
  let result = templateStore.templates

  if (selectedCategory.value) {
    result = result.filter(t => t.category === selectedCategory.value)
  }

  if (searchKeyword.value) {
    const keyword = searchKeyword.value.toLowerCase()
    result = result.filter(t =>
      t.name.toLowerCase().includes(keyword) ||
      t.topic.toLowerCase().includes(keyword)
    )
  }

  return result
})

// 初始化加载
onMounted(() => {
  if (props.serverId) {
    templateStore.loadTemplates(props.serverId)
  }
})

// 监听serverId变化
watch(() => props.serverId, (newId) => {
  if (newId) {
    templateStore.loadTemplates(newId)
  }
})

// 获取payload类型对应的标签颜色
function getPayloadTypeTag(type: string) {
  switch (type) {
    case 'json': return 'primary'
    case 'hex': return 'warning'
    default: return 'info'
  }
}

// 格式化payload预览
function formatPayload(payload: string): string {
  const maxLen = 100
  if (payload.length > maxLen) {
    return payload.substring(0, maxLen) + '...'
  }
  return payload
}

// 搜索处理
function handleSearch() {
  // 本地过滤，无需额外操作
}

// 创建模板
function handleCreate() {
  editingTemplate.value = null
  showDialog.value = true
}

// 使用模板
async function handleUse(template: CommandTemplate) {
  try {
    const used = await templateStore.useTemplate(template.id!)
    emit('use', used)
  } catch (error) {
    ElMessage.error(t('errors.loadFailed'))
  }
}

// 下拉菜单命令
async function handleCommand(command: string, template: CommandTemplate) {
  switch (command) {
    case 'edit':
      editingTemplate.value = template
      showDialog.value = true
      break
    case 'duplicate':
      try {
        const { value: newName } = await ElMessageBox.prompt(
          t('template.namePlaceholder'),
          t('common.copy'),
          {
            inputValue: `${template.name} - Copy`,
            inputPattern: /\S+/,
            inputErrorMessage: t('errors.inputName')
          }
        )
        await templateStore.duplicateTemplate(template.id!, newName)
        ElMessage.success(t('server.duplicateSuccess'))
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(t('errors.saveFailed'))
        }
      }
      break
    case 'delete':
      try {
        await ElMessageBox.confirm(
          t('template.deleteConfirm'),
          t('template.deleteTitle'),
          { type: 'warning', confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel') }
        )
        await templateStore.deleteTemplate(template.id!)
        ElMessage.success(t('template.deleteSuccess'))
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
  editingTemplate.value = null
  ElMessage.success(t('success.saved'))
}
</script>

<style scoped lang="scss">
.template-drawer {
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

.category-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.category-tag {
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    opacity: 0.8;
  }
}

.template-cards {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-right: 4px;
}

.template-card {
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
  color: var(--app-text-color);
}

.more-btn {
  opacity: 0.6;
  transition: opacity 0.2s;

  &:hover {
    opacity: 1;
  }
}

.card-topic {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--app-text-secondary);

  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    background-color: var(--sidebar-hover);
    padding: 2px 6px;
    border-radius: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
}

.card-payload {
  margin-bottom: 12px;
  
  pre {
    margin: 0;
    padding: 10px;
    background-color: var(--sidebar-hover);
    border-radius: 6px;
    font-family: 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    line-height: 1.5;
    color: var(--app-text-color);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 80px;
  }
}

.card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.meta-item {
  font-size: 12px;
  color: var(--app-text-secondary);

  &.retain {
    color: var(--status-connected);
    font-weight: 500;
  }
}
</style>
