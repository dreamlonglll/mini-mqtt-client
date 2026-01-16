import { createI18n, type LocaleMessages, type VueMessageType } from 'vue-i18n'
import yaml from 'js-yaml'

// 导入 YAML 翻译文件作为原始字符串
import zhCNYaml from './locales/zh-CN.yaml?raw'
import enUSYaml from './locales/en-US.yaml?raw'

// 解析 YAML 为对象
const zhCN = yaml.load(zhCNYaml) as LocaleMessages<VueMessageType>
const enUS = yaml.load(enUSYaml) as LocaleMessages<VueMessageType>

export type Locale = 'auto' | 'zh-CN' | 'en-US'
export type ActualLocale = 'zh-CN' | 'en-US'

// 支持的语言列表
export const supportedLocales: ActualLocale[] = ['zh-CN', 'en-US']

// 获取系统语言
export function getSystemLocale(): ActualLocale {
  const lang = navigator.language
  // 匹配中文
  if (lang.startsWith('zh')) {
    return 'zh-CN'
  }
  // 其他语言默认英文
  return 'en-US'
}

// 获取实际使用的语言
export function getActualLocale(locale: Locale): ActualLocale {
  if (locale === 'auto') {
    return getSystemLocale()
  }
  return locale
}

// 创建 i18n 实例
const i18n = createI18n({
  legacy: false, // 使用 Composition API 模式
  locale: 'zh-CN', // 默认语言，会在 app 初始化时更新
  fallbackLocale: 'en-US', // 回退语言
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS,
  },
})

export default i18n
