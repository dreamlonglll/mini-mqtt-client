<template>
  <div class="quick-send-panel">
    <!-- 面板头部 -->
    <div class="panel-header">
      <div class="header-title">
        <el-icon><Lightning /></el-icon>
        <span>{{ $t('publish.send') }}</span>
      </div>
      <el-button text size="small" @click="$emit('manage')">
        <el-icon><Setting /></el-icon>
        {{ $t('publish.openTemplates') }}
      </el-button>
    </div>

    <!-- 搜索框 -->
    <div class="search-wrapper">
      <el-input
        v-model="searchKeyword"
        :placeholder="$t('template.searchPlaceholder')"
        :prefix-icon="Search"
        clearable
        size="small"
      />
    </div>

    <!-- 分类标签 -->
    <div v-if="categories.length > 0" class="category-tabs">
      <el-scrollbar>
        <div class="tabs-inner">
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
      </el-scrollbar>
    </div>

    <!-- 模板列表 -->
    <div class="template-list">
      <el-scrollbar>
        <div class="list-inner">
          <div
            v-for="template in displayedTemplates"
            :key="template.id"
            class="template-item"
            @click="handleSend(template)"
          >
            <div class="item-main">
              <div class="item-header">
                <span class="item-name">{{ template.name }}</span>
                <el-tag size="small" :type="getPayloadTypeTag(template.payload_type)">
                  {{ template.payload_type.toUpperCase() }}
                </el-tag>
              </div>
              <div class="item-topic">
                <el-icon><Position /></el-icon>
                <code>{{ template.topic }}</code>
              </div>
            </div>
            <div class="item-meta">
              <span class="use-count">
                <el-icon><TrendCharts /></el-icon>
                {{ template.use_count }}
              </span>
            </div>
          </div>

          <!-- 空状态 -->
          <el-empty
            v-if="displayedTemplates.length === 0"
            :description="$t('template.noTemplate')"
            :image-size="60"
          >
            <el-button type="primary" size="small" @click="$emit('manage')">
              {{ $t('template.addTemplate') }}
            </el-button>
          </el-empty>
        </div>
      </el-scrollbar>
    </div>

    <!-- 最近使用 -->
    <div v-if="recentTemplates.length > 0" class="recent-section">
      <div class="recent-header">
        <el-icon><Clock /></el-icon>
        <span>Recent</span>
      </div>
      <div class="recent-list">
        <el-tag
          v-for="template in recentTemplates"
          :key="template.id"
          class="recent-tag"
          effect="plain"
          @click="handleSend(template)"
        >
          {{ template.name }}
        </el-tag>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import {
  Search,
  Setting,
  Position,
  Clock,
  TrendCharts
} from '@element-plus/icons-vue'
// 使用 SVG 替代 Lightning 图标
import { Promotion as Lightning } from '@element-plus/icons-vue'
import { useTemplateStore, type CommandTemplate } from '@/stores/template'

const { t } = useI18n()

const props = defineProps<{
  serverId: number
}>()

const emit = defineEmits<{
  send: [template: CommandTemplate]
  manage: []
}>()

const templateStore = useTemplateStore()

const searchKeyword = ref('')
const selectedCategory = ref<string | null>(null)

const categories = computed(() => templateStore.categories)

// 显示的模板列表
const displayedTemplates = computed(() => {
  let result = templateStore.templates

  // 按分类筛选
  if (selectedCategory.value) {
    result = result.filter(t => t.category === selectedCategory.value)
  }

  // 按关键词搜索
  if (searchKeyword.value) {
    const keyword = searchKeyword.value.toLowerCase()
    result = result.filter(t =>
      t.name.toLowerCase().includes(keyword) ||
      t.topic.toLowerCase().includes(keyword)
    )
  }

  // 限制显示数量
  return result.slice(0, 20)
})

// 最近使用的模板
const recentTemplates = computed(() => templateStore.recentTemplates.slice(0, 5))

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

// 发送模板
async function handleSend(template: CommandTemplate) {
  try {
    const used = await templateStore.useTemplate(template.id!)
    emit('send', used)
    ElMessage.success(`${t('template.loadSuccess')}: ${template.name}`)
  } catch (error) {
    ElMessage.error(t('errors.loadFailed'))
  }
}
</script>

<style scoped lang="scss">
.quick-send-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--card-bg);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  overflow: hidden;
}

// 面板头部
.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--app-border-color);
  flex-shrink: 0;
}

.header-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: var(--app-text-color);
}

// 搜索框
.search-wrapper {
  padding: 12px 16px 8px;
  flex-shrink: 0;
}

// 分类标签
.category-tabs {
  padding: 0 16px 12px;
  flex-shrink: 0;
}

.tabs-inner {
  display: flex;
  gap: 8px;
  white-space: nowrap;
}

.category-tag {
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    opacity: 0.8;
  }
}

// 模板列表
.template-list {
  flex: 1;
  overflow: hidden;
}

.list-inner {
  padding: 0 16px;
}

.template-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  margin-bottom: 6px;
  border-radius: 6px;
  background-color: var(--sidebar-hover);
  cursor: pointer;
  transition: all 0.2s ease;

  &:hover {
    background-color: var(--sidebar-active);
    transform: translateX(2px);
  }

  &:last-child {
    margin-bottom: 0;
  }
}

.item-main {
  flex: 1;
  min-width: 0;
}

.item-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.item-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-topic {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--app-text-secondary);

  code {
    font-family: 'Fira Code', 'Consolas', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.item-meta {
  flex-shrink: 0;
  margin-left: 8px;
}

.use-count {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 11px;
  color: var(--app-text-secondary);
}

// 最近使用
.recent-section {
  padding: 12px 16px;
  border-top: 1px solid var(--app-border-color);
  flex-shrink: 0;
}

.recent-header {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 10px;
  font-size: 12px;
  font-weight: 500;
  color: var(--app-text-secondary);
}

.recent-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.recent-tag {
  cursor: pointer;
  transition: all 0.2s ease;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;

  &:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.1);
  }
}
</style>
