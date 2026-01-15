use crate::log::{LogEntry, LogManager};
use tauri::State;

/// 写入错误日志
#[tauri::command]
pub fn write_error_log(entry: LogEntry, log_manager: State<'_, LogManager>) -> Result<(), String> {
    log_manager.write_log(&entry)
}

/// 获取最近的日志
#[tauri::command]
pub fn get_recent_logs(limit: Option<usize>, log_manager: State<'_, LogManager>) -> Result<Vec<String>, String> {
    log_manager.get_recent_logs(limit.unwrap_or(100))
}

/// 获取日志目录路径
#[tauri::command]
pub fn get_log_dir(log_manager: State<'_, LogManager>) -> String {
    log_manager.get_log_dir().to_string_lossy().to_string()
}

/// 清空所有日志
#[tauri::command]
pub fn clear_logs(log_manager: State<'_, LogManager>) -> Result<(), String> {
    log_manager.clear_logs()
}
