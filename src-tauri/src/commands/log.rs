use crate::log::{LogEntry, LogManager};
use tauri::State;

/// 批量写入错误日志（前端每 500ms 或攒满一批调用一次）
#[tauri::command]
pub async fn write_error_logs(
    entries: Vec<LogEntry>,
    log_manager: State<'_, LogManager>,
) -> Result<(), String> {
    log_manager.write_logs(&entries)
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
