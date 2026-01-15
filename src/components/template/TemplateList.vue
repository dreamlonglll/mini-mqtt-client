<template>
  <div class="template-list-container">
    <!-- 工具栏 -->
    <div class="template-toolbar">
      <div class="toolbar-left">
        <el-input
          v-model="searchKeyword"
          placeholder="搜索模板..."
          :prefix-icon="Search"
          clearable
          class="search-input"
          @input="handleSearch"
        />
        <el-select
          v-model="selectedCategory"
          placeholder="全部分类"
          clearable
          class="category-select"
          @change="handleCategoryChange"
        >
          <el-option
            v-for="category in categories"
            :key="category"
            :label="category"
            :value="category"
          />
        </el-select>
      </div>
      <div class="toolbar-right">
        <el-button type="primary" :icon="Plus" @click="handleCreate">
          新建模板
        </el-button>
        <el-dropdown @command="handleCommand">
          <el-button :icon="MoreFilled" />
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="import" :icon="Upload">
                导入模板
              </el-dropdown-item>
              <el-dropdown-item command="export" :icon="Download">
                导出模板
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </div>

    <!-- 常用模板快速访问 -->
    <div v-if="frequentTemplates.length > 0" class="frequent-section">
      <div class="section-header">
        <el-icon><Star /></el-icon>
        <span>常用模板</span>
      </div>
      <div class="frequent-tags">
        <el-tag
          v-for="template in frequentTemplates"
          :key="template.id"
          class="frequent-tag"
          effect="plain"
          @click="handleQuickSend(template)"
        >
          <span class="tag-name">{{ template.name }}</span>
          <span class="tag-count">({{ template.use_count }})</span>
        </el-tag>
      </div>
    </div>

    <!-- 模板列表 -->
    <div class="template-table-wrapper app-card">
      <el-table
        :data="filteredTemplates"
        v-loading="loading"
        stripe
        class="template-table"
        @row-dblclick="handleQuickSend"
      >
        <el-table-column prop="name" label="模板名称" min-width="140">
          <template #default="{ row }">
            <div class="template-name-cell">
              <span class="name">{{ row.name }}</span>
              <span v-if="row.description" class="desc text-ellipsis">
                {{ row.description }}
              </span>
            </div>
          </template>
        </el-table-column>

        <el-table-column prop="topic" label="主题" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            <code class="topic-code">{{ row.topic }}</code>
          </template>
        </el-table-column>

        <el-table-column prop="payload_type" label="类型" width="80" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="getPayloadTypeTag(row.payload_type)">
              {{ row.payload_type.toUpperCase() }}
            </el-tag>
          </template>
        </el-table-column>

        <el-table-column prop="qos" label="QoS" width="60" align="center">
          <template #default="{ row }">
            <span class="qos-badge">{{ row.qos }}</span>
          </template>
        </el-table-column>

        <el-table-column prop="retain" label="Retain" width="70" align="center">
          <template #default="{ row }">
            <el-icon v-if="row.retain" class="retain-icon active"><Check /></el-icon>
            <el-icon v-else class="retain-icon"><Close /></el-icon>
          </template>
        </el-table-column>

        <el-table-column prop="category" label="分类" width="100">
          <template #default="{ row }">
            <el-tag v-if="row.category" size="small" effect="plain" type="info">
              {{ row.category }}
            </el-tag>
            <span v-else class="no-category">-</span>
          </template>
        </el-table-column>

        <el-table-column prop="use_count" label="使用次数" width="90" align="center">
          <template #default="{ row }">
            <span class="use-count">{{ row.use_count }}</span>
          </template>
        </el-table-column>

        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <div class="action-buttons">
              <el-tooltip content="发送" placement="top">
                <el-button
                  size="small"
                  type="primary"
                  :icon="Promotion"
                  circle
                  @click="handleQuickSend(row)"
                />
              </el-tooltip>
              <el-tooltip content="编辑" placement="top">
                <el-button
                  size="small"
                  :icon="Edit"
                  circle
                  @click="handleEdit(row)"
                />
              </el-tooltip>
              <el-tooltip content="复制" placement="top">
                <el-button
                  size="small"
                  :icon="CopyDocument"
                  circle
                  @click="handleDuplicate(row)"
                />
              </el-tooltip>
              <el-tooltip content="删除" placement="top">
                <el-button
                  size="small"
                  type="danger"
                  :icon="Delete"
                  circle
                  @click="handleDelete(row)"
                />
              </el-tooltip>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <!-- 空状态 -->
      <el-empty
        v-if="!loading && filteredTemplates.length === 0"
        description="暂无模板"
        :image-size="80"
      >
        <el-button type="primary" @click="handleCreate">
          创建第一个模板
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

    <!-- 导入对话框 -->
    <el-dialog v-model="showImportDialog" title="导入模板" width="500px">
      <el-upload
        ref="uploadRef"
        :auto-upload="false"
        :limit="1"
        accept=".json"
        drag
        @change="handleFileChange"
      >
        <el-icon class="upload-icon"><Upload /></el-icon>
        <div class="upload-text">
          将JSON文件拖到此处，或<em>点击上传</em>
        </div>
        <template #tip>
          <div class="upload-tip">
            请选择之前导出的JSON模板文件
          </div>
        </template>
      </el-upload>
      <template #footer>
        <el-button @click="showImportDialog = false">取消</el-button>
        <el-button type="primary" @click="handleImport" :loading="importing">
          导入
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Search,
  Plus,
  MoreFilled,
  Upload,
  Download,
  Star,
  Promotion,
  Edit,
  CopyDocument,
  Delete,
  Check,
  Close
} from '@element-plus/icons-vue'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'
import TemplateDialog from './TemplateDialog.vue'

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
const showImportDialog = ref(false)
const editingTemplate = ref<CommandTemplate | null>(null)
const importFileContent = ref('')
const importing = ref(false)

const loading = computed(() => templateStore.loading)
const filteredTemplates = computed(() => templateStore.filteredTemplates)
const frequentTemplates = computed(() => templateStore.frequentTemplates)
const categories = computed(() => templateStore.categories)

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

// 搜索处理
function handleSearch() {
  templateStore.setSearchKeyword(searchKeyword.value)
}

// 分类筛选
function handleCategoryChange() {
  templateStore.setCategory(selectedCategory.value)
}

// 创建模板
function handleCreate() {
  editingTemplate.value = null
  showDialog.value = true
}

// 编辑模板
function handleEdit(template: CommandTemplate) {
  editingTemplate.value = template
  showDialog.value = true
}

// 快速发送
async function handleQuickSend(template: CommandTemplate) {
  try {
    const used = await templateStore.useTemplate(template.id!)
    emit('use', used)
    ElMessage.success(`已加载模板: ${template.name}`)
  } catch (error) {
    ElMessage.error('加载模板失败')
  }
}

// 复制模板
async function handleDuplicate(template: CommandTemplate) {
  try {
    const { value: newName } = await ElMessageBox.prompt(
      '请输入新模板名称',
      '复制模板',
      {
        inputValue: `${template.name} - 副本`,
        inputPattern: /\S+/,
        inputErrorMessage: '名称不能为空'
      }
    )
    await templateStore.duplicateTemplate(template.id!, newName)
    ElMessage.success('模板已复制')
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('复制模板失败')
    }
  }
}

// 删除模板
async function handleDelete(template: CommandTemplate) {
  try {
    await ElMessageBox.confirm(
      `确定要删除模板 "${template.name}" 吗？`,
      '删除确认',
      { type: 'warning' }
    )
    await templateStore.deleteTemplate(template.id!)
    ElMessage.success('模板已删除')
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.error('删除模板失败')
    }
  }
}

// 保存成功回调
function handleSaved() {
  showDialog.value = false
  editingTemplate.value = null
  ElMessage.success('模板已保存')
}

// 下拉菜单命令处理
function handleCommand(command: string) {
  if (command === 'import') {
    showImportDialog.value = true
  } else if (command === 'export') {
    handleExport()
  }
}

// 导出模板（使用浏览器下载）
async function handleExport() {
  try {
    const json = await templateStore.exportTemplates(props.serverId)
    
    // 使用浏览器原生下载
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = 'mqtt-templates.json'
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    URL.revokeObjectURL(url)
    
    ElMessage.success('模板已导出')
  } catch (error) {
    ElMessage.error('导出失败')
  }
}

// 文件选择变化
function handleFileChange(file: any) {
  const reader = new FileReader()
  reader.onload = (e) => {
    importFileContent.value = e.target?.result as string
  }
  reader.readAsText(file.raw)
}

// 导入模板
async function handleImport() {
  if (!importFileContent.value) {
    ElMessage.warning('请先选择文件')
    return
  }

  importing.value = true
  try {
    const count = await templateStore.importTemplates(props.serverId, importFileContent.value)
    ElMessage.success(`成功导入 ${count} 个模板`)
    showImportDialog.value = false
    importFileContent.value = ''
  } catch (error) {
    ElMessage.error('导入失败: ' + error)
  } finally {
    importing.value = false
  }
}
</script>

<style scoped lang="scss">
.template-list-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}

// 工具栏
.template-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  gap: 12px;
  flex: 1;
}

.search-input {
  max-width: 280px;
}

.category-select {
  width: 140px;
}

.toolbar-right {
  display: flex;
  gap: 8px;
}

// 常用模板区
.frequent-section {
  flex-shrink: 0;
  padding: 12px 16px;
  background-color: var(--primary-light);
  border-radius: 8px;
  border: 1px solid var(--app-border-color);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 10px;
  font-size: 13px;
  font-weight: 500;
  color: var(--primary-color);
}

.frequent-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.frequent-tag {
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .tag-name {
    font-weight: 500;
  }

  .tag-count {
    margin-left: 4px;
    opacity: 0.7;
    font-size: 12px;
  }
}

// 模板表格
.template-table-wrapper {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.template-table {
  flex: 1;
}

.template-name-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;

  .name {
    font-weight: 500;
    color: var(--app-text-color);
  }

  .desc {
    font-size: 12px;
    color: var(--app-text-secondary);
    max-width: 200px;
  }
}

.topic-code {
  font-family: 'Fira Code', 'Consolas', monospace;
  font-size: 12px;
  padding: 2px 6px;
  background-color: var(--sidebar-hover);
  border-radius: 4px;
  color: var(--app-text-color);
}

.qos-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background-color: var(--primary-light);
  color: var(--primary-color);
  font-size: 12px;
  font-weight: 600;
}

.retain-icon {
  font-size: 16px;
  color: var(--app-text-secondary);

  &.active {
    color: var(--status-connected);
  }
}

.no-category {
  color: var(--app-text-secondary);
}

.use-count {
  font-weight: 500;
  color: var(--app-text-secondary);
}

.action-buttons {
  display: flex;
  gap: 4px;
  justify-content: center;
}

// 导入对话框
.upload-icon {
  font-size: 48px;
  color: var(--app-text-secondary);
  margin-bottom: 8px;
}

.upload-text {
  color: var(--app-text-secondary);

  em {
    color: var(--primary-color);
    font-style: normal;
  }
}

.upload-tip {
  font-size: 12px;
  color: var(--app-text-secondary);
  margin-top: 8px;
  text-align: center;
}

:deep(.el-upload-dragger) {
  padding: 32px;
}

:deep(.el-table) {
  --el-table-header-bg-color: var(--sidebar-hover);
}
</style>
