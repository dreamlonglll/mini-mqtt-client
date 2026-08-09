use chrono::Local;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

/// 每写入多少行触发一次旧日志清理（避免每行都全目录扫描）
const CLEANUP_SAMPLE_INTERVAL: u32 = 100;

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub r#type: String,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: String,
}

/// 持久化的写入器状态
struct WriterState {
    writer: Option<BufWriter<File>>,
    /// writer 当前对应的日志文件路径（跨天时重新打开）
    path: PathBuf,
    /// 当前文件已写入字节数（自行累计，避免每行 stat）
    size: u64,
    /// 自上次清理后写入的行数
    writes_since_cleanup: u32,
}

/// 日志管理器
pub struct LogManager {
    log_dir: PathBuf,
    max_log_files: usize,
    max_file_size: u64, // bytes
    state: Mutex<WriterState>,
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
            state: Mutex::new(WriterState {
                writer: None,
                path: PathBuf::new(),
                size: 0,
                writes_since_cleanup: 0,
            }),
        })
    }

    /// 获取当前日志文件路径
    fn get_current_log_file(&self) -> PathBuf {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.log_dir.join(format!("error-{}.log", today))
    }

    /// 确保 writer 已打开并指向今天的日志文件
    fn ensure_writer(&self, state: &mut WriterState) -> Result<(), String> {
        let expected = self.get_current_log_file();
        if state.writer.is_some() && state.path == expected {
            return Ok(());
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&expected)
            .map_err(|e| format!("Failed to open log file: {}", e))?;
        state.size = file.metadata().map(|m| m.len()).unwrap_or(0);
        state.writer = Some(BufWriter::new(file));
        state.path = expected;
        Ok(())
    }

    /// 写入单条日志条目
    pub fn write_log(&self, entry: &LogEntry) -> Result<(), String> {
        self.write_logs(std::slice::from_ref(entry))
    }

    /// 批量写入日志条目（一次加锁、一次 flush）
    pub fn write_logs(&self, entries: &[LogEntry]) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut state = self.state.lock();
        self.ensure_writer(&mut state)?;

        let mut written: u64 = 0;
        {
            let writer = state.writer.as_mut().expect("writer ensured above");
            for entry in entries {
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
                writer
                    .write_all(log_line.as_bytes())
                    .map_err(|e| format!("Failed to write log: {}", e))?;
                written += log_line.len() as u64;
            }
            writer
                .flush()
                .map_err(|e| format!("Failed to flush log: {}", e))?;
        }
        state.size += written;
        state.writes_since_cleanup += entries.len() as u32;

        // 超过大小上限则轮转（先关闭句柄再重命名）
        if state.size > self.max_file_size {
            state.writer = None;
            let path = state.path.clone();
            self.rotate_log_file(&path)?;
            state.size = 0;
        }

        // 采样触发旧日志清理，而非每行都扫描目录
        if state.writes_since_cleanup >= CLEANUP_SAMPLE_INTERVAL {
            state.writes_since_cleanup = 0;
            drop(state);
            let _ = self.cleanup_old_logs();
        }

        Ok(())
    }

    /// 轮转日志文件
    fn rotate_log_file(&self, log_file: &PathBuf) -> Result<(), String> {
        let timestamp = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
        let rotated_name = log_file.with_extension(format!("{}.log", timestamp));
        fs::rename(log_file, rotated_name).map_err(|e| format!("Failed to rotate log file: {}", e))
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

    /// 读取最近的日志条目（从文件尾部按块回读，不载入全文件）
    pub fn get_recent_logs(&self, limit: usize) -> Result<Vec<String>, String> {
        let log_file = self.get_current_log_file();

        if !log_file.exists() {
            return Ok(Vec::new());
        }

        let mut file =
            File::open(&log_file).map_err(|e| format!("Failed to read log file: {}", e))?;
        let len = file
            .metadata()
            .map_err(|e| e.to_string())?
            .len();

        const CHUNK: u64 = 64 * 1024;
        let mut buf: Vec<u8> = Vec::new();
        let mut pos = len;

        // 从尾部回读，直到覆盖 limit 行或到达文件头
        while pos > 0 {
            let newline_count = buf.iter().filter(|&&b| b == b'\n').count();
            if newline_count > limit {
                break;
            }
            let read_size = CHUNK.min(pos);
            pos -= read_size;
            file.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
            let mut chunk = vec![0u8; read_size as usize];
            file.read_exact(&mut chunk).map_err(|e| e.to_string())?;
            chunk.extend_from_slice(&buf);
            buf = chunk;
        }

        let text = String::from_utf8_lossy(&buf);
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        if lines.len() > limit {
            lines = lines.split_off(lines.len() - limit);
        }
        Ok(lines)
    }

    /// 清空所有日志
    pub fn clear_logs(&self) -> Result<(), String> {
        // 先释放当前写入句柄，避免占用待删除的文件
        {
            let mut state = self.state.lock();
            state.writer = None;
            state.size = 0;
        }
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
