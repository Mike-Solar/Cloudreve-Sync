use crate::core::db::LogRow;
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub task_id: String,
    pub level: LogLevel,
    pub event: String,
    pub detail: String,
    pub created_at_ms: i64,
}

impl LogEntry {
    pub fn new(task_id: &str, level: LogLevel, event: &str, detail: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            level,
            event: event.to_string(),
            detail: detail.to_string(),
            created_at_ms: Utc::now().timestamp_millis(),
        }
    }

    pub fn to_row(&self) -> LogRow {
        LogRow {
            task_id: self.task_id.clone(),
            level: self.level.as_str().to_string(),
            event: self.event.clone(),
            detail: self.detail.clone(),
            created_at_ms: self.created_at_ms,
        }
    }
}

#[derive(Clone)]
pub struct LogStore {
    db_path: PathBuf,
}

impl LogStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn append(&self, conn: &mut Connection, entry: &LogEntry) -> Result<(), Box<dyn Error>> {
        conn.execute(
            "INSERT INTO logs (task_id, level, event, detail, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                entry.task_id.clone(),
                entry.level.as_str().to_string(),
                entry.event.clone(),
                entry.detail.clone(),
                entry.created_at_ms,
            ),
        )?;
        Ok(())
    }
}
