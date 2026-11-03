//! Error log files (`<app data>/logs/error-YYYY-MM-DD.log`), ported from the
//! Tauri backend's `LogManager`.

use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum LogKind {
    Mqtt,
    Script,
    Storage,
    App,
}

impl LogKind {
    fn label(self) -> &'static str {
        match self {
            LogKind::Mqtt => "MQTT",
            LogKind::Script => "SCRIPT",
            LogKind::Storage => "STORAGE",
            LogKind::App => "APP",
        }
    }
}

pub struct LogManager {
    log_dir: PathBuf,
    max_log_files: usize,
    max_file_size: u64,
}

impl LogManager {
    pub fn new(app_dir: &Path) -> Self {
        let log_dir = app_dir.join("logs");
        let _ = fs::create_dir_all(&log_dir);
        Self {
            log_dir,
            max_log_files: 10,
            max_file_size: 5_000_000,
        }
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    fn current_log_file(&self) -> PathBuf {
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.log_dir.join(format!("error-{today}.log"))
    }

    pub fn write(&self, kind: LogKind, message: &str, details: Option<&str>) {
        if let Err(err) = self.try_write(kind, message, details) {
            log::warn!("failed to write log: {err}");
        }
    }

    fn try_write(&self, kind: LogKind, message: &str, details: Option<&str>) -> Result<(), String> {
        fs::create_dir_all(&self.log_dir).map_err(|e| e.to_string())?;
        let log_file = self.current_log_file();
        if let Ok(meta) = fs::metadata(&log_file)
            && meta.len() > self.max_file_size
        {
            let stamp = Local::now().format("%Y-%m-%d_%H%M%S").to_string();
            let _ = fs::rename(&log_file, log_file.with_extension(format!("{stamp}.log")));
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|e| format!("无法打开日志文件: {e}"))?;
        let line = format!(
            "[{}] [{}] {}{}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            kind.label(),
            message,
            details.map(|d| format!(" | Details: {d}")).unwrap_or_default()
        );
        file.write_all(line.as_bytes())
            .map_err(|e| format!("写入日志失败: {e}"))?;
        self.cleanup_old_logs();
        Ok(())
    }

    fn cleanup_old_logs(&self) {
        let Ok(entries) = fs::read_dir(&self.log_dir) else {
            return;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
            .collect();
        if files.len() <= self.max_log_files {
            return;
        }
        files.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        let excess = files.len() - self.max_log_files;
        for entry in files.into_iter().take(excess) {
            let _ = fs::remove_file(entry.path());
        }
    }

    /// Last `limit` lines of today's log.
    pub fn recent_logs(&self, limit: usize) -> Vec<String> {
        let Ok(file) = File::open(self.current_log_file()) else {
            return Vec::new();
        };
        let lines: Vec<String> = BufReader::new(file).lines().map_while(Result::ok).collect();
        let start = lines.len().saturating_sub(limit);
        lines[start..].to_vec()
    }

    pub fn clear(&self) -> Result<(), String> {
        for entry in fs::read_dir(&self.log_dir).map_err(|e| e.to_string())?.flatten() {
            if entry.path().extension().is_some_and(|ext| ext == "log") {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}
