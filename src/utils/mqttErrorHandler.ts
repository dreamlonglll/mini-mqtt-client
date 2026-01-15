import { errorHandler, ErrorType } from './errorHandler'

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
 */
const mqttErrorMap: Record<string, MqttErrorInfo> = {
  'connection_refused': {
    code: 'CONNECTION_REFUSED',
    message: '连接被拒绝',
    suggestion: '请检查服务器地址和端口是否正确'
  },
  'connection_timeout': {
    code: 'CONNECTION_TIMEOUT',
    message: '连接超时',
    suggestion: '请检查网络连接或服务器是否可用'
  },
  'connection refused': {
    code: 'CONNECTION_REFUSED',
    message: '连接被拒绝',
    suggestion: '请检查服务器地址和端口是否正确'
  },
  'timeout': {
    code: 'CONNECTION_TIMEOUT',
    message: '连接超时',
    suggestion: '请检查网络连接或服务器是否可用'
  },
  'auth_failed': {
    code: 'AUTH_FAILED',
    message: '认证失败',
    suggestion: '请检查用户名和密码是否正确'
  },
  'authentication': {
    code: 'AUTH_FAILED',
    message: '认证失败',
    suggestion: '请检查用户名和密码是否正确'
  },
  'bad user name or password': {
    code: 'AUTH_FAILED',
    message: '用户名或密码错误',
    suggestion: '请检查用户名和密码是否正确'
  },
  'not_authorized': {
    code: 'NOT_AUTHORIZED',
    message: '未授权',
    suggestion: '当前用户没有该操作权限'
  },
  'not authorized': {
    code: 'NOT_AUTHORIZED',
    message: '未授权',
    suggestion: '当前用户没有该操作权限'
  },
  'topic_invalid': {
    code: 'TOPIC_INVALID',
    message: '主题格式无效',
    suggestion: '请检查主题格式是否符合MQTT规范'
  },
  'invalid topic': {
    code: 'TOPIC_INVALID',
    message: '主题格式无效',
    suggestion: '请检查主题格式是否符合MQTT规范'
  },
  'payload_too_large': {
    code: 'PAYLOAD_TOO_LARGE',
    message: '消息体过大',
    suggestion: '请减小消息体大小'
  },
  'packet too large': {
    code: 'PAYLOAD_TOO_LARGE',
    message: '消息包过大',
    suggestion: '请减小消息体大小'
  },
  'disconnected': {
    code: 'DISCONNECTED',
    message: '连接已断开',
    suggestion: '请重新连接服务器'
  },
  'network': {
    code: 'NETWORK_ERROR',
    message: '网络错误',
    suggestion: '请检查网络连接是否正常'
  },
  'io error': {
    code: 'IO_ERROR',
    message: 'IO错误',
    suggestion: '请检查网络连接或重试'
  },
  'broker unavailable': {
    code: 'BROKER_UNAVAILABLE',
    message: '服务器不可用',
    suggestion: '请检查服务器是否正常运行'
  },
  'client identifier not valid': {
    code: 'INVALID_CLIENT_ID',
    message: 'Client ID 无效',
    suggestion: '请检查 Client ID 格式是否正确'
  },
  'protocol': {
    code: 'PROTOCOL_ERROR',
    message: '协议错误',
    suggestion: '请检查 MQTT 协议版本是否匹配'
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
  for (const [key, info] of Object.entries(mqttErrorMap)) {
    if (lowerError.includes(key)) {
      if (!silent) {
        errorHandler.handle(`${info.message}: ${info.suggestion}`, ErrorType.MQTT)
      }
      return info
    }
  }

  // 未知 MQTT 错误
  const unknownError: MqttErrorInfo = {
    code: 'UNKNOWN',
    message: error,
    suggestion: '请查看详细日志获取更多信息'
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
  const statusMap: Record<MqttConnectionStatus, { text: string; type: 'success' | 'warning' | 'danger' | 'info' }> = {
    'disconnected': { text: '未连接', type: 'info' },
    'connecting': { text: '连接中...', type: 'warning' },
    'connected': { text: '已连接', type: 'success' },
    'reconnecting': { text: '重连中...', type: 'warning' },
    'error': { text: '连接错误', type: 'danger' }
  }
  
  return statusMap[status] || { text: status, type: 'info' }
}

/**
 * 获取 MQTT 错误建议
 * @param errorCode 错误代码
 */
export function getMqttErrorSuggestion(errorCode: string): string {
  for (const info of Object.values(mqttErrorMap)) {
    if (info.code === errorCode) {
      return info.suggestion
    }
  }
  return '请查看详细日志获取更多信息'
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
