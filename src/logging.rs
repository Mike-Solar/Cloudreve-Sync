use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        let now: DateTime<Local> = Local::now();
        Self {
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            level,
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct LogStore {
    path: PathBuf,
}

impl LogStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, entry: &LogEntry) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<LogEntry>, Box<dyn Error>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path)?;
        let mut out = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: LogEntry = serde_json::from_str(line)?;
            out.push(entry);
        }
        Ok(out)
    }
}
