import { createI18n, type LocaleMessages, type VueMessageType } from 'vue-i18n'

// 语言 YAML 由 @intlify/unplugin-vue-i18n 在构建期预编译为消息函数，
// 运行期不再解析 YAML，也不需要打包消息编译器
import zhCN from './locales/zh-CN.yaml'
import enUS from './locales/en-US.yaml'

export type Locale = 'auto' | 'zh-CN' | 'en-US'
export type ActualLocale = 'zh-CN' | 'en-US'

// 各语言的消息对象（供键集合对齐测试使用）
// 显式标注类型，避免 vue-i18n 从预编译产物反推出深层消息 schema（TS2589）
export const localeMessages: Record<ActualLocale, LocaleMessages<VueMessageType>> = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

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
  messages: localeMessages,
})

/**
 * 以运行期字符串键取翻译
 *
 * 工具模块的键是动态拼接的，直接调用 `i18n.global.t` 会触发 vue-i18n
 * 对字面量键的深度类型推导（TS2589）。此处统一收口并在调用时求值，
 * 因此文案始终跟随当前语言，不会被模块加载期快照。
 */
export function translate(key: string, named?: Record<string, unknown>): string {
  const t = i18n.global.t as unknown as (
    key: string,
    named?: Record<string, unknown>
  ) => string
  return named ? t(key, named) : t(key)
}

export default i18n
