use crate::log::{LogEntry, LogManager};
use tauri::State;

/// 写入错误日志（async：在 tokio worker 上执行文件 IO，不阻塞主线程）
#[tauri::command]
pub async fn write_error_log(
    entry: LogEntry,
    log_manager: State<'_, LogManager>,
) -> Result<(), String> {
    log_manager.write_log(&entry)
}

/// 批量写入错误日志（前端每 500ms 或攒满一批调用一次）
#[tauri::command]
pub async fn write_error_logs(
    entries: Vec<LogEntry>,
    log_manager: State<'_, LogManager>,
) -> Result<(), String> {
    log_manager.write_logs(&entries)
}

/// 获取最近的日志
#[tauri::command]
pub async fn get_recent_logs(
    limit: Option<usize>,
    log_manager: State<'_, LogManager>,
) -> Result<Vec<String>, String> {
    log_manager.get_recent_logs(limit.unwrap_or(100))
}

/// 获取日志目录路径
#[tauri::command]
pub fn get_log_dir(log_manager: State<'_, LogManager>) -> String {
    log_manager.get_log_dir().to_string_lossy().to_string()
}

/// 清空所有日志
#[tauri::command]
pub async fn clear_logs(log_manager: State<'_, LogManager>) -> Result<(), String> {
    log_manager.clear_logs()
}
