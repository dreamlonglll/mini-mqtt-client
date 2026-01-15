use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub r#type: String,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: String,
}

/// 日志管理器
pub struct LogManager {
    log_dir: PathBuf,
    max_log_files: usize,
    max_file_size: u64, // bytes
}

impl LogManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;

        let log_dir = app_dir.join("logs");
        fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;

        Ok(Self {
            log_dir,
            max_log_files: 10,        // 最多保留 10 个日志文件
            max_file_size: 5_000_000, // 每个文件最大 5MB
        })
    }

    /// 获取当前日志文件路径
    fn get_current_log_file(&self) -> PathBuf {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.log_dir.join(format!("error-{}.log", today))
    }

    /// 写入日志条目
    pub fn write_log(&self, entry: &LogEntry) -> Result<(), String> {
        let log_file = self.get_current_log_file();

        // 检查文件大小，如果超过限制则轮转
        if log_file.exists() {
            if let Ok(metadata) = fs::metadata(&log_file) {
                if metadata.len() > self.max_file_size {
                    self.rotate_log_file(&log_file)?;
                }
            }
        }

        // 打开或创建日志文件
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|e| format!("无法打开日志文件: {}", e))?;

        // 格式化日志条目
        let log_line = format!(
            "[{}] [{}] {}{}\n",
            entry.timestamp,
            entry.r#type.to_uppercase(),
            entry.message,
            entry
                .details
                .as_ref()
                .map(|d| format!(" | Details: {}", d))
                .unwrap_or_default()
        );

        file.write_all(log_line.as_bytes())
            .map_err(|e| format!("写入日志失败: {}", e))?;

        // 清理旧日志文件
        self.cleanup_old_logs()?;

        Ok(())
    }

    /// 轮转日志文件
    fn rotate_log_file(&self, log_file: &PathBuf) -> Result<(), String> {
        let timestamp = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
        let rotated_name = log_file.with_extension(format!("{}.log", timestamp));
        fs::rename(log_file, rotated_name).map_err(|e| format!("轮转日志文件失败: {}", e))
    }

    /// 清理旧日志文件
    fn cleanup_old_logs(&self) -> Result<(), String> {
        let mut log_files: Vec<_> = fs::read_dir(&self.log_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "log")
                    .unwrap_or(false)
            })
            .collect();

        if log_files.len() <= self.max_log_files {
            return Ok(());
        }

        // 按修改时间排序
        log_files.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        // 删除最旧的文件
        let to_delete = log_files.len() - self.max_log_files;
        for entry in log_files.into_iter().take(to_delete) {
            let _ = fs::remove_file(entry.path());
        }

        Ok(())
    }

    /// 获取日志目录路径
    pub fn get_log_dir(&self) -> &PathBuf {
        &self.log_dir
    }

    /// 读取最近的日志条目
    pub fn get_recent_logs(&self, limit: usize) -> Result<Vec<String>, String> {
        let log_file = self.get_current_log_file();

        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&log_file).map_err(|e| format!("无法读取日志文件: {}", e))?;

        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .filter_map(|line| line.ok())
            .collect();

        // 返回最后 limit 行
        let start = if lines.len() > limit {
            lines.len() - limit
        } else {
            0
        };

        Ok(lines[start..].to_vec())
    }

    /// 清空所有日志
    pub fn clear_logs(&self) -> Result<(), String> {
        for entry in fs::read_dir(&self.log_dir).map_err(|e| e.to_string())? {
            if let Ok(entry) = entry {
                if entry.path().extension().map(|ext| ext == "log").unwrap_or(false) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }
}
