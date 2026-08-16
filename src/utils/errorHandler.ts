import { ElNotification } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'
import { translate } from '@/i18n'

/**
 * 错误类型枚举
 */
export enum ErrorType {
  NETWORK = 'network',
  MQTT = 'mqtt',
  DATABASE = 'database',
  VALIDATION = 'validation',
  SCRIPT = 'script',
  UNKNOWN = 'unknown'
}

/**
 * 应用错误接口
 */
export interface AppError {
  type: ErrorType
  message: string
  details?: any
  timestamp: Date
}

/**
 * 错误类型对应的 i18n 标题键
 */
const errorTitleKeys: Record<ErrorType, string> = {
  [ErrorType.NETWORK]: 'errorTypes.network',
  [ErrorType.MQTT]: 'errorTypes.mqtt',
  [ErrorType.DATABASE]: 'errorTypes.database',
  [ErrorType.VALIDATION]: 'errorTypes.validation',
  [ErrorType.SCRIPT]: 'errorTypes.script',
  [ErrorType.UNKNOWN]: 'errorTypes.unknown'
}

/**
 * 取错误类型的标题（在调用时求值，跟随当前语言）
 */
function errorTitleOf(type: ErrorType): string {
  return translate(errorTitleKeys[type] ?? errorTitleKeys[ErrorType.UNKNOWN])
}

/**
 * 待写入文件的日志条目
 */
interface LogEntryPayload {
  type: string
  message: string
  details: string | null
  timestamp: string
}

/** 同一错误的去重时间窗口（毫秒） */
const DEDUPE_WINDOW_MS = 5000
/** 日志批量落盘间隔（毫秒） */
const LOG_FLUSH_INTERVAL_MS = 500
/** 单批日志最大条数（达到即立即落盘） */
const LOG_FLUSH_MAX_ENTRIES = 100

/**
 * 全局错误处理器类
 */
class ErrorHandler {
  private errors: AppError[] = []
  private maxErrors = 100
  private logToFileEnabled = true

  // 同一 type+message 在时间窗口内去重计数，避免高频消息流上的坏脚本刷爆日志
  private recentErrors = new Map<string, { count: number; firstAt: number }>()
  // 待批量写入文件的日志队列
  private pendingLogs: LogEntryPayload[] = []
  private logFlushTimer: ReturnType<typeof setTimeout> | null = null

  /**
   * 处理错误
   * @param error 错误对象
   * @param type 错误类型
   * @param silent 是否静默处理（不显示通知）
   */
  handle(error: unknown, type: ErrorType = ErrorType.UNKNOWN, silent: boolean = false): AppError {
    const appError = this.createAppError(error, type)

    // 时间窗口内的重复错误只计数，不再打日志 / 弹通知 / 写文件
    const key = `${type}:${appError.message}`
    const now = Date.now()
    const recent = this.recentErrors.get(key)
    if (recent && now - recent.firstAt < DEDUPE_WINDOW_MS) {
      recent.count++
      return appError
    }
    // 上一窗口有积累计数时补记一条汇总
    if (recent && recent.count > 1) {
      this.enqueueLog({
        type: appError.type,
        message: `${appError.message}（该错误在过去 ${Math.round(DEDUPE_WINDOW_MS / 1000)} 秒内共发生 ${recent.count} 次）`,
        details: null,
        timestamp: new Date().toISOString(),
      })
    }
    this.recentErrors.set(key, { count: 1, firstAt: now })
    this.pruneRecentErrors(now)

    this.logError(appError)
    this.storeError(appError)

    if (!silent) {
      this.notifyUser(appError)
    }

    // 排队批量写入日志文件
    if (this.logToFileEnabled) {
      this.enqueueLog({
        type: appError.type,
        message: appError.message,
        details: appError.details ? JSON.stringify(appError.details) : null,
        timestamp: appError.timestamp.toISOString(),
      })
    }

    return appError
  }

  /**
   * 清理过期的去重记录，防止 Map 无限增长
   */
  private pruneRecentErrors(now: number): void {
    if (this.recentErrors.size <= 200) return
    for (const [key, value] of this.recentErrors) {
      if (now - value.firstAt > DEDUPE_WINDOW_MS) {
        this.recentErrors.delete(key)
      }
    }
  }

  /**
   * 日志入队，按时间/数量批量落盘
   */
  private enqueueLog(entry: LogEntryPayload): void {
    this.pendingLogs.push(entry)
    if (this.pendingLogs.length >= LOG_FLUSH_MAX_ENTRIES) {
      this.flushLogs()
      return
    }
    if (!this.logFlushTimer) {
      this.logFlushTimer = setTimeout(() => this.flushLogs(), LOG_FLUSH_INTERVAL_MS)
    }
  }

  /**
   * 将排队的日志一次性写入文件
   */
  private flushLogs(): void {
    if (this.logFlushTimer) {
      clearTimeout(this.logFlushTimer)
      this.logFlushTimer = null
    }
    if (this.pendingLogs.length === 0) return
    const entries = this.pendingLogs.splice(0)
    invoke('write_error_logs', { entries }).catch((e) => {
      // 避免循环调用，只在控制台输出
      console.error('写入日志文件失败:', e)
    })
  }

  /**
   * 创建应用错误对象
   */
  private createAppError(error: unknown, type: ErrorType): AppError {
    let message = '发生未知错误'
    let details: any

    if (error instanceof Error) {
      message = error.message
      details = {
        name: error.name,
        stack: error.stack
      }
    } else if (typeof error === 'string') {
      message = error
    } else if (error && typeof error === 'object') {
      message = (error as any).message || JSON.stringify(error)
      details = error
    }

    return {
      type,
      message,
      details,
      timestamp: new Date()
    }
  }

  /**
   * 控制台输出错误
   */
  private logError(error: AppError): void {
    console.error(`[${error.type.toUpperCase()}] ${error.message}`, error.details)
  }

  /**
   * 存储错误到内存
   */
  private storeError(error: AppError): void {
    this.errors.unshift(error)
    if (this.errors.length > this.maxErrors) {
      this.errors = this.errors.slice(0, this.maxErrors)
    }
  }

  /**
   * 向用户显示错误通知
   */
  private notifyUser(error: AppError): void {
    ElNotification({
      title: errorTitleOf(error.type),
      message: error.message,
      type: 'error',
      duration: 5000
    })
  }

  /**
   * 获取所有错误记录
   */
  getErrors(): AppError[] {
    return [...this.errors]
  }

  /**
   * 获取指定类型的错误
   */
  getErrorsByType(type: ErrorType): AppError[] {
    return this.errors.filter(e => e.type === type)
  }

  /**
   * 清空错误记录
   */
  clearErrors(): void {
    this.errors = []
  }

  /**
   * 设置是否启用日志文件记录
   */
  setLogToFileEnabled(enabled: boolean): void {
    this.logToFileEnabled = enabled
  }

  /**
   * 获取最近的错误
   */
  getLatestError(): AppError | null {
    return this.errors.length > 0 ? this.errors[0] : null
  }

  /**
   * 获取错误数量
   */
  getErrorCount(): number {
    return this.errors.length
  }
}

// 导出单例实例
export const errorHandler = new ErrorHandler()

/**
 * 设置全局错误捕获
 * 在 main.ts 中调用
 */
export function setupGlobalErrorHandler(): void {
  // 捕获未处理的 JavaScript 错误
  window.addEventListener('error', (event) => {
    errorHandler.handle(event.error || event.message, ErrorType.UNKNOWN)
  })

  // 捕获未处理的 Promise 拒绝
  window.addEventListener('unhandledrejection', (event) => {
    errorHandler.handle(event.reason, ErrorType.UNKNOWN)
  })

  console.log('[ErrorHandler] 全局错误处理器已初始化')
}

/**
 * 便捷函数：处理网络错误
 */
export function handleNetworkError(error: unknown, silent = false): AppError {
  return errorHandler.handle(error, ErrorType.NETWORK, silent)
}

/**
 * 便捷函数：处理 MQTT 错误
 */
export function handleMqttError(error: unknown, silent = false): AppError {
  return errorHandler.handle(error, ErrorType.MQTT, silent)
}

/**
 * 便捷函数：处理数据库错误
 */
export function handleDatabaseError(error: unknown, silent = false): AppError {
  return errorHandler.handle(error, ErrorType.DATABASE, silent)
}

/**
 * 便捷函数：处理验证错误
 */
export function handleValidationError(error: unknown, silent = false): AppError {
  return errorHandler.handle(error, ErrorType.VALIDATION, silent)
}

/**
 * 便捷函数：处理脚本错误
 */
export function handleScriptError(error: unknown, silent = false): AppError {
  return errorHandler.handle(error, ErrorType.SCRIPT, silent)
}
