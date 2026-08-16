import { errorHandler, ErrorType } from './errorHandler'
import i18n, { translate } from '@/i18n'

/**
 * MQTT 错误信息接口
 */
export interface MqttErrorInfo {
  code: string
  message: string
  suggestion: string
}

/**
 * MQTT 连接状态
 */
export type MqttConnectionStatus = 
  | 'disconnected' 
  | 'connecting' 
  | 'connected' 
  | 'reconnecting' 
  | 'error'

/**
 * MQTT 错误映射表
 *
 * key 为错误原文（小写）的匹配子串，value 的 i18nKey 指向语言文件中的
 * `mqttErrors.*` 节点（含 message / suggestion 两个叶子）。
 * 文案在匹配命中时才求值，因此跟随当前语言。
 *
 * 注意：第二批之后 Rust 侧的入口校验返回中文错误消息，这些消息不会命中
 * 下面任何一个英文子串，会走末尾的"未知错误"分支原样透传——这正是期望行为。
 */
const mqttErrorMap: Record<string, { code: string; i18nKey: string }> = {
  'connection_refused': { code: 'CONNECTION_REFUSED', i18nKey: 'mqttErrors.connectionRefused' },
  'connection_timeout': { code: 'CONNECTION_TIMEOUT', i18nKey: 'mqttErrors.connectionTimeout' },
  'connection refused': { code: 'CONNECTION_REFUSED', i18nKey: 'mqttErrors.connectionRefused' },
  'timeout': { code: 'CONNECTION_TIMEOUT', i18nKey: 'mqttErrors.connectionTimeout' },
  'auth_failed': { code: 'AUTH_FAILED', i18nKey: 'mqttErrors.authFailed' },
  'authentication': { code: 'AUTH_FAILED', i18nKey: 'mqttErrors.authFailed' },
  'bad user name or password': { code: 'AUTH_FAILED', i18nKey: 'mqttErrors.badCredentials' },
  'badusernamepassword': { code: 'AUTH_FAILED', i18nKey: 'mqttErrors.badCredentials' },
  'not_authorized': { code: 'NOT_AUTHORIZED', i18nKey: 'mqttErrors.notAuthorized' },
  'not authorized': { code: 'NOT_AUTHORIZED', i18nKey: 'mqttErrors.notAuthorized' },
  'notauthorized': { code: 'NOT_AUTHORIZED', i18nKey: 'mqttErrors.notAuthorized' },
  'topic_invalid': { code: 'TOPIC_INVALID', i18nKey: 'mqttErrors.topicInvalid' },
  'invalid topic': { code: 'TOPIC_INVALID', i18nKey: 'mqttErrors.topicInvalid' },
  'payload_too_large': { code: 'PAYLOAD_TOO_LARGE', i18nKey: 'mqttErrors.payloadTooLarge' },
  'packet too large': { code: 'PAYLOAD_TOO_LARGE', i18nKey: 'mqttErrors.packetTooLarge' },
  'disconnected': { code: 'DISCONNECTED', i18nKey: 'mqttErrors.disconnected' },
  'network': { code: 'NETWORK_ERROR', i18nKey: 'mqttErrors.network' },
  'io error': { code: 'IO_ERROR', i18nKey: 'mqttErrors.io' },
  'broker unavailable': { code: 'BROKER_UNAVAILABLE', i18nKey: 'mqttErrors.brokerUnavailable' },
  'serviceunavailable': { code: 'BROKER_UNAVAILABLE', i18nKey: 'mqttErrors.brokerUnavailable' },
  'client identifier not valid': { code: 'INVALID_CLIENT_ID', i18nKey: 'mqttErrors.invalidClientId' },
  'badclientid': { code: 'INVALID_CLIENT_ID', i18nKey: 'mqttErrors.invalidClientId' },
  'protocol': { code: 'PROTOCOL_ERROR', i18nKey: 'mqttErrors.protocol' }
}

/** 把映射表条目展开成带文案的错误信息（调用时求值，跟随当前语言） */
function resolveErrorInfo(entry: { code: string; i18nKey: string }): MqttErrorInfo {
  return {
    code: entry.code,
    message: translate(`${entry.i18nKey}.message`),
    suggestion: translate(`${entry.i18nKey}.suggestion`)
  }
}

/**
 * 处理 MQTT 错误
 * @param error 错误字符串
 * @param silent 是否静默处理
 * @returns MQTT 错误信息
 */
export function handleMqttError(error: string, silent: boolean = false): MqttErrorInfo {
  const lowerError = error.toLowerCase()

  // 尝试匹配已知错误
  for (const [key, entry] of Object.entries(mqttErrorMap)) {
    if (lowerError.includes(key)) {
      const info = resolveErrorInfo(entry)
      if (!silent) {
        errorHandler.handle(`${info.message}: ${info.suggestion}`, ErrorType.MQTT)
      }
      return info
    }
  }

  // 未知 MQTT 错误：原样透传后端消息（第二批起后端已返回本地化的中文提示）
  const unknownError: MqttErrorInfo = {
    code: 'UNKNOWN',
    message: error,
    suggestion: translate('mqttErrors.unknownSuggestion')
  }

  if (!silent) {
    errorHandler.handle(error, ErrorType.MQTT)
  }

  return unknownError
}

/**
 * 检查 MQTT 是否已连接
 */
export function isMqttConnected(status: MqttConnectionStatus): boolean {
  return status === 'connected'
}

/**
 * 获取 MQTT 状态显示信息
 */
export function getMqttStatusInfo(status: MqttConnectionStatus): { 
  text: string
  type: 'success' | 'warning' | 'danger' | 'info' 
  icon?: string
} {
  const t = i18n.global.t
  const statusMap: Record<MqttConnectionStatus, { text: string; type: 'success' | 'warning' | 'danger' | 'info' }> = {
    'disconnected': { text: t('header.status.disconnected'), type: 'info' },
    'connecting': { text: t('header.status.connecting'), type: 'warning' },
    'connected': { text: t('header.status.connected'), type: 'success' },
    'reconnecting': { text: t('header.status.connecting'), type: 'warning' },
    'error': { text: t('header.status.error'), type: 'danger' }
  }
  
  return statusMap[status] || { text: status, type: 'info' }
}

/**
 * 获取 MQTT 错误建议
 * @param errorCode 错误代码
 */
export function getMqttErrorSuggestion(errorCode: string): string {
  for (const entry of Object.values(mqttErrorMap)) {
    if (entry.code === errorCode) {
      return translate(`${entry.i18nKey}.suggestion`)
    }
  }
  return translate('mqttErrors.unknownSuggestion')
}

/**
 * 验证 MQTT Topic 格式
 * @param topic Topic 字符串
 * @returns 验证结果
 */
export function validateMqttTopic(topic: string): { valid: boolean; error?: string } {
  if (!topic || topic.trim() === '') {
    return { valid: false, error: 'Topic 不能为空' }
  }
  
  if (topic.length > 65535) {
    return { valid: false, error: 'Topic 长度超过限制' }
  }
  
  // 检查是否包含空字符
  if (topic.includes('\0')) {
    return { valid: false, error: 'Topic 不能包含空字符' }
  }
  
  // 检查是否以 / 开头（不推荐但允许）
  // if (topic.startsWith('/')) {
  //   return { valid: true, warning: 'Topic 以 / 开头不推荐' }
  // }
  
  return { valid: true }
}

/**
 * 验证 MQTT 发布 Topic（不允许通配符）
 * @param topic Topic 字符串
 */
export function validatePublishTopic(topic: string): { valid: boolean; error?: string } {
  const baseValidation = validateMqttTopic(topic)
  if (!baseValidation.valid) {
    return baseValidation
  }
  
  // 发布主题不能包含通配符
  if (topic.includes('+') || topic.includes('#')) {
    return { valid: false, error: '发布主题不能包含通配符 (+ 或 #)' }
  }
  
  return { valid: true }
}

/**
 * 验证 MQTT 订阅 Topic（允许通配符）
 * @param topic Topic 字符串
 */
export function validateSubscribeTopic(topic: string): { valid: boolean; error?: string } {
  const baseValidation = validateMqttTopic(topic)
  if (!baseValidation.valid) {
    return baseValidation
  }
  
  // 检查 # 通配符位置（只能在末尾）
  const hashIndex = topic.indexOf('#')
  if (hashIndex !== -1) {
    if (hashIndex !== topic.length - 1) {
      return { valid: false, error: '# 通配符只能在主题末尾' }
    }
    if (hashIndex > 0 && topic[hashIndex - 1] !== '/') {
      return { valid: false, error: '# 通配符前必须是 /' }
    }
  }
  
  // 检查 + 通配符位置（必须占据整个层级）
  const parts = topic.split('/')
  for (const part of parts) {
    if (part.includes('+') && part !== '+') {
      return { valid: false, error: '+ 通配符必须占据整个层级' }
    }
  }
  
  return { valid: true }
}
