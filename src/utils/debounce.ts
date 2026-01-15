/**
 * 防抖函数
 * 在指定时间内多次触发只执行最后一次
 * @param fn 要执行的函数
 * @param delay 延迟时间（毫秒）
 */
export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null
  
  return function (this: any, ...args: Parameters<T>) {
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    
    timeoutId = setTimeout(() => {
      fn.apply(this, args)
      timeoutId = null
    }, delay)
  }
}

/**
 * 节流函数
 * 在指定时间内只执行一次
 * @param fn 要执行的函数
 * @param limit 时间间隔（毫秒）
 */
export function throttle<T extends (...args: any[]) => any>(
  fn: T,
  limit: number
): (...args: Parameters<T>) => void {
  let inThrottle = false
  let lastArgs: Parameters<T> | null = null
  
  return function (this: any, ...args: Parameters<T>) {
    if (!inThrottle) {
      fn.apply(this, args)
      inThrottle = true
      
      setTimeout(() => {
        inThrottle = false
        if (lastArgs) {
          fn.apply(this, lastArgs)
          lastArgs = null
        }
      }, limit)
    } else {
      lastArgs = args
    }
  }
}

/**
 * 带立即执行选项的防抖函数
 * @param fn 要执行的函数
 * @param delay 延迟时间（毫秒）
 * @param immediate 是否立即执行
 */
export function debounceImmediate<T extends (...args: any[]) => any>(
  fn: T,
  delay: number,
  immediate: boolean = false
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null
  
  return function (this: any, ...args: Parameters<T>) {
    const callNow = immediate && !timeoutId
    
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    
    timeoutId = setTimeout(() => {
      timeoutId = null
      if (!immediate) {
        fn.apply(this, args)
      }
    }, delay)
    
    if (callNow) {
      fn.apply(this, args)
    }
  }
}

/**
 * 带取消功能的防抖函数
 */
export interface DebouncedFunction<T extends (...args: any[]) => any> {
  (...args: Parameters<T>): void
  cancel: () => void
}

export function debounceWithCancel<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): DebouncedFunction<T> {
  let timeoutId: ReturnType<typeof setTimeout> | null = null
  
  const debounced = function (this: any, ...args: Parameters<T>) {
    if (timeoutId) {
      clearTimeout(timeoutId)
    }
    
    timeoutId = setTimeout(() => {
      fn.apply(this, args)
      timeoutId = null
    }, delay)
  } as DebouncedFunction<T>
  
  debounced.cancel = () => {
    if (timeoutId) {
      clearTimeout(timeoutId)
      timeoutId = null
    }
  }
  
  return debounced
}
