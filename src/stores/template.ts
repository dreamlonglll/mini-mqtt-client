import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface CommandTemplate {
  id?: number
  server_id: number
  name: string
  topic: string
  payload: string
  payload_type: 'json' | 'hex' | 'text'
  qos: 0 | 1 | 2
  retain: boolean
  description?: string
  category?: string
  use_count: number
  last_used_at?: string
  created_at?: string
  updated_at?: string
}

export interface CreateTemplateRequest {
  server_id: number
  name: string
  topic: string
  payload: string
  payload_type: string
  qos: number
  retain: boolean
  description?: string
  category?: string
}

export interface UpdateTemplateRequest {
  id: number
  name?: string
  topic?: string
  payload?: string
  payload_type?: string
  qos?: number
  retain?: boolean
  description?: string
  category?: string
}

export const useTemplateStore = defineStore('template', () => {
  const templates = ref<CommandTemplate[]>([])
  const categories = ref<string[]>([])
  const loading = ref(false)
  const currentServerId = ref<number | null>(null)
  const selectedCategory = ref<string | null>(null)
  const searchKeyword = ref('')

  // 计算属性：按分类过滤的模板
  const filteredTemplates = computed(() => {
    let result = templates.value

    if (selectedCategory.value) {
      result = result.filter(t => t.category === selectedCategory.value)
    }

    if (searchKeyword.value) {
      const keyword = searchKeyword.value.toLowerCase()
      result = result.filter(t =>
        t.name.toLowerCase().includes(keyword) ||
        t.topic.toLowerCase().includes(keyword) ||
        (t.description?.toLowerCase().includes(keyword) ?? false)
      )
    }

    return result
  })

  // 计算属性：常用模板（前5个）
  const frequentTemplates = computed(() => {
    return [...templates.value]
      .filter(t => t.use_count > 0)
      .sort((a, b) => b.use_count - a.use_count)
      .slice(0, 5)
  })

  // 计算属性：最近使用的模板
  const recentTemplates = computed(() => {
    return [...templates.value]
      .filter(t => t.last_used_at)
      .sort((a, b) => {
        const dateA = new Date(a.last_used_at!).getTime()
        const dateB = new Date(b.last_used_at!).getTime()
        return dateB - dateA
      })
      .slice(0, 10)
  })

  // 加载模板列表
  async function loadTemplates(serverId: number) {
    loading.value = true
    currentServerId.value = serverId
    try {
      templates.value = await invoke<CommandTemplate[]>('list_templates', { serverId })
      await loadCategories(serverId)
    } catch (error) {
      console.error('加载模板失败:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  // 加载分类列表
  async function loadCategories(serverId: number) {
    try {
      categories.value = await invoke<string[]>('get_template_categories', { serverId })
    } catch (error) {
      console.error('加载分类失败:', error)
    }
  }

  // 创建模板
  async function createTemplate(request: CreateTemplateRequest): Promise<number> {
    try {
      const id = await invoke<number>('create_template', { request })
      if (currentServerId.value === request.server_id) {
        await loadTemplates(request.server_id)
      }
      return id
    } catch (error) {
      console.error('创建模板失败:', error)
      throw error
    }
  }

  // 更新模板
  async function updateTemplate(request: UpdateTemplateRequest) {
    try {
      await invoke('update_template', { request })
      if (currentServerId.value) {
        await loadTemplates(currentServerId.value)
      }
    } catch (error) {
      console.error('更新模板失败:', error)
      throw error
    }
  }

  // 删除模板
  async function deleteTemplate(id: number) {
    try {
      await invoke('delete_template', { id })
      templates.value = templates.value.filter(t => t.id !== id)
      // 重新计算分类列表（只保留仍有模板的分类）
      updateCategories()
    } catch (error) {
      console.error('删除模板失败:', error)
      throw error
    }
  }

  // 更新分类列表（基于当前模板计算）
  function updateCategories() {
    const usedCategories = new Set<string>()
    for (const template of templates.value) {
      if (template.category) {
        usedCategories.add(template.category)
      }
    }
    categories.value = Array.from(usedCategories).sort()
    
    // 如果当前选中的分类已不存在，清除选择
    if (selectedCategory.value && !usedCategories.has(selectedCategory.value)) {
      selectedCategory.value = null
    }
  }

  // 使用模板
  async function useTemplate(id: number): Promise<CommandTemplate> {
    try {
      const template = await invoke<CommandTemplate>('use_template', { id })
      // 更新本地使用次数
      const index = templates.value.findIndex(t => t.id === id)
      if (index !== -1) {
        templates.value[index].use_count++
        templates.value[index].last_used_at = new Date().toISOString()
      }
      
      // 返回原始模板（保留 {{变量名}} 占位符）
      // 变量替换在实际发送消息时进行
      return template
    } catch (error) {
      console.error('使用模板失败:', error)
      throw error
    }
  }

  // 搜索模板
  async function searchTemplates(serverId: number, keyword: string) {
    try {
      templates.value = await invoke<CommandTemplate[]>('search_templates', {
        serverId,
        keyword
      })
    } catch (error) {
      console.error('搜索模板失败:', error)
      throw error
    }
  }

  // 导出模板
  async function exportTemplates(serverId: number): Promise<string> {
    try {
      return await invoke<string>('export_templates', { serverId })
    } catch (error) {
      console.error('导出模板失败:', error)
      throw error
    }
  }

  // 导入模板
  async function importTemplates(serverId: number, jsonData: string): Promise<number> {
    try {
      const count = await invoke<number>('import_templates', { serverId, jsonData })
      await loadTemplates(serverId)
      return count
    } catch (error) {
      console.error('导入模板失败:', error)
      throw error
    }
  }

  // 复制模板
  async function duplicateTemplate(id: number, newName: string): Promise<number> {
    try {
      const newId = await invoke<number>('duplicate_template', { id, newName })
      if (currentServerId.value) {
        await loadTemplates(currentServerId.value)
      }
      return newId
    } catch (error) {
      console.error('复制模板失败:', error)
      throw error
    }
  }

  // 设置筛选分类
  function setCategory(category: string | null) {
    selectedCategory.value = category
  }

  // 设置搜索关键词
  function setSearchKeyword(keyword: string) {
    searchKeyword.value = keyword
  }

  // 清空筛选
  function clearFilters() {
    selectedCategory.value = null
    searchKeyword.value = ''
  }

  return {
    templates,
    categories,
    loading,
    currentServerId,
    selectedCategory,
    searchKeyword,
    filteredTemplates,
    frequentTemplates,
    recentTemplates,
    loadTemplates,
    loadCategories,
    createTemplate,
    updateTemplate,
    deleteTemplate,
    useTemplate,
    searchTemplates,
    exportTemplates,
    importTemplates,
    duplicateTemplate,
    setCategory,
    setSearchKeyword,
    clearFilters
  }
})
