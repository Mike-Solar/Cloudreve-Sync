use chrono::Utc;
use serde::Serialize;
use rusqlite::{params, params_from_iter, Connection, Result};
use rusqlite::types::Value;

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub task_id: String,
    pub base_url: String,
    pub local_root: String,
    pub remote_root_uri: String,
    pub device_id: String,
    pub mode: String,
    pub settings_json: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub account_key: String,
    pub base_url: String,
    pub email: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct EntryRow {
    pub task_id: String,
    pub local_relpath: String,
    pub cloud_file_id: String,
    pub cloud_uri: String,
    pub last_local_mtime_ms: i64,
    pub last_local_sha256: String,
    pub last_remote_mtime_ms: i64,
    pub last_remote_sha256: String,
    pub last_sync_ts_ms: i64,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct TombstoneRow {
    pub task_id: String,
    pub cloud_file_id: String,
    pub local_relpath: String,
    pub deleted_at_ms: i64,
    pub origin: String,
}

#[derive(Debug, Clone)]
pub struct ConflictRow {
    pub task_id: String,
    pub original_relpath: String,
    pub conflict_relpath: String,
    pub created_at_ms: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub task_id: String,
    pub level: String,
    pub event: String,
    pub detail: String,
    pub created_at_ms: i64,
}

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tasks (
            task_id TEXT PRIMARY KEY,
            base_url TEXT NOT NULL,
            local_root TEXT NOT NULL,
            remote_root_uri TEXT NOT NULL,
            device_id TEXT NOT NULL,
            mode TEXT NOT NULL,
            settings_json TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS accounts (
            account_key TEXT PRIMARY KEY,
            base_url TEXT NOT NULL,
            email TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entries (
            task_id TEXT NOT NULL,
            local_relpath TEXT NOT NULL,
            cloud_file_id TEXT NOT NULL,
            cloud_uri TEXT NOT NULL,
            last_local_mtime_ms INTEGER NOT NULL,
            last_local_sha256 TEXT NOT NULL,
            last_remote_mtime_ms INTEGER NOT NULL,
            last_remote_sha256 TEXT NOT NULL,
            last_sync_ts_ms INTEGER NOT NULL,
            state TEXT NOT NULL,
            PRIMARY KEY (task_id, local_relpath)
        );

        CREATE TABLE IF NOT EXISTS tombstones (
            task_id TEXT NOT NULL,
            cloud_file_id TEXT NOT NULL,
            local_relpath TEXT NOT NULL,
            deleted_at_ms INTEGER NOT NULL,
            origin TEXT NOT NULL,
            PRIMARY KEY (task_id, local_relpath)
        );

        CREATE TABLE IF NOT EXISTS conflicts (
            task_id TEXT NOT NULL,
            original_relpath TEXT NOT NULL,
            conflict_relpath TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            reason TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            level TEXT NOT NULL,
            event TEXT NOT NULL,
            detail TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        "#,
    )?;
    Ok(())
}

pub fn upsert_account(conn: &Connection, account: &AccountRow) -> Result<()> {
    conn.execute(
        "INSERT INTO accounts (account_key, base_url, email, created_at_ms) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(account_key) DO UPDATE SET base_url=excluded.base_url, email=excluded.email",
        params![
            account.account_key,
            account.base_url,
            account.email,
            account.created_at_ms
        ],
    )?;
    Ok(())
}

pub fn list_accounts(conn: &Connection) -> Result<Vec<AccountRow>> {
    let mut stmt = conn.prepare(
        "SELECT account_key, base_url, email, created_at_ms FROM accounts ORDER BY created_at_ms DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(AccountRow {
            account_key: row.get(0)?,
            base_url: row.get(1)?,
            email: row.get(2)?,
            created_at_ms: row.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete_all_accounts(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM accounts", [])?;
    Ok(())
}

pub fn create_task(conn: &Connection, task: &TaskRow) -> Result<()> {
    conn.execute(
        "INSERT INTO tasks (task_id, base_url, local_root, remote_root_uri, device_id, mode, settings_json, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            task.task_id,
            task.base_url,
            task.local_root,
            task.remote_root_uri,
            task.device_id,
            task.mode,
            task.settings_json,
            task.created_at_ms
        ],
    )?;
    Ok(())
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<TaskRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, base_url, local_root, remote_root_uri, device_id, mode, settings_json, created_at_ms FROM tasks ORDER BY created_at_ms DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TaskRow {
            task_id: row.get(0)?,
            base_url: row.get(1)?,
            local_root: row.get(2)?,
            remote_root_uri: row.get(3)?,
            device_id: row.get(4)?,
            mode: row.get(5)?,
            settings_json: row.get(6)?,
            created_at_ms: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete_task(conn: &Connection, task_id: &str) -> Result<()> {
    conn.execute("DELETE FROM entries WHERE task_id = ?1", params![task_id])?;
    conn.execute("DELETE FROM tombstones WHERE task_id = ?1", params![task_id])?;
    conn.execute("DELETE FROM conflicts WHERE task_id = ?1", params![task_id])?;
    conn.execute("DELETE FROM logs WHERE task_id = ?1", params![task_id])?;
    conn.execute("DELETE FROM tasks WHERE task_id = ?1", params![task_id])?;
    Ok(())
}

pub fn upsert_entry(conn: &Connection, entry: &EntryRow) -> Result<()> {
    conn.execute(
        "INSERT INTO entries (task_id, local_relpath, cloud_file_id, cloud_uri, last_local_mtime_ms, last_local_sha256, last_remote_mtime_ms, last_remote_sha256, last_sync_ts_ms, state) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(task_id, local_relpath) DO UPDATE SET cloud_file_id=excluded.cloud_file_id, cloud_uri=excluded.cloud_uri, last_local_mtime_ms=excluded.last_local_mtime_ms, last_local_sha256=excluded.last_local_sha256, last_remote_mtime_ms=excluded.last_remote_mtime_ms, last_remote_sha256=excluded.last_remote_sha256, last_sync_ts_ms=excluded.last_sync_ts_ms, state=excluded.state",
        params![
            entry.task_id,
            entry.local_relpath,
            entry.cloud_file_id,
            entry.cloud_uri,
            entry.last_local_mtime_ms,
            entry.last_local_sha256,
            entry.last_remote_mtime_ms,
            entry.last_remote_sha256,
            entry.last_sync_ts_ms,
            entry.state
        ],
    )?;
    Ok(())
}

pub fn list_entries_by_task(conn: &Connection, task_id: &str) -> Result<Vec<EntryRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, local_relpath, cloud_file_id, cloud_uri, last_local_mtime_ms, last_local_sha256, last_remote_mtime_ms, last_remote_sha256, last_sync_ts_ms, state FROM entries WHERE task_id = ?1",
    )?;
    let rows = stmt.query_map(params![task_id], |row| {
        Ok(EntryRow {
            task_id: row.get(0)?,
            local_relpath: row.get(1)?,
            cloud_file_id: row.get(2)?,
            cloud_uri: row.get(3)?,
            last_local_mtime_ms: row.get(4)?,
            last_local_sha256: row.get(5)?,
            last_remote_mtime_ms: row.get(6)?,
            last_remote_sha256: row.get(7)?,
            last_sync_ts_ms: row.get(8)?,
            state: row.get(9)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert_tombstone(conn: &Connection, tombstone: &TombstoneRow) -> Result<()> {
    conn.execute(
        "INSERT INTO tombstones (task_id, cloud_file_id, local_relpath, deleted_at_ms, origin) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(task_id, local_relpath) DO UPDATE SET cloud_file_id=excluded.cloud_file_id, deleted_at_ms=excluded.deleted_at_ms, origin=excluded.origin",
        params![
            tombstone.task_id,
            tombstone.cloud_file_id,
            tombstone.local_relpath,
            tombstone.deleted_at_ms,
            tombstone.origin
        ],
    )?;
    Ok(())
}

pub fn list_tombstones(conn: &Connection, task_id: &str) -> Result<Vec<TombstoneRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, cloud_file_id, local_relpath, deleted_at_ms, origin FROM tombstones WHERE task_id = ?1",
    )?;
    let rows = stmt.query_map(params![task_id], |row| {
        Ok(TombstoneRow {
            task_id: row.get(0)?,
            cloud_file_id: row.get(1)?,
            local_relpath: row.get(2)?,
            deleted_at_ms: row.get(3)?,
            origin: row.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert_conflict(conn: &Connection, conflict: &ConflictRow) -> Result<()> {
    conn.execute(
        "INSERT INTO conflicts (task_id, original_relpath, conflict_relpath, created_at_ms, reason) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            conflict.task_id,
            conflict.original_relpath,
            conflict.conflict_relpath,
            conflict.created_at_ms,
            conflict.reason
        ],
    )?;
    Ok(())
}

pub fn delete_conflict(conn: &Connection, task_id: &str, conflict_relpath: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM conflicts WHERE task_id = ?1 AND conflict_relpath = ?2",
        params![task_id, conflict_relpath],
    )?;
    Ok(())
}

pub fn list_conflicts(conn: &Connection, task_id: Option<&str>) -> Result<Vec<ConflictRow>> {
    let mut out = Vec::new();
    if let Some(task_id) = task_id {
        let mut stmt = conn.prepare(
            "SELECT task_id, original_relpath, conflict_relpath, created_at_ms, reason FROM conflicts WHERE task_id = ?1 ORDER BY created_at_ms DESC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(ConflictRow {
                task_id: row.get(0)?,
                original_relpath: row.get(1)?,
                conflict_relpath: row.get(2)?,
                created_at_ms: row.get(3)?,
                reason: row.get(4)?,
            })
        })?;
        for row in rows {
            out.push(row?);
        }
        return Ok(out);
    }
    let mut stmt = conn.prepare(
        "SELECT task_id, original_relpath, conflict_relpath, created_at_ms, reason FROM conflicts ORDER BY created_at_ms DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ConflictRow {
            task_id: row.get(0)?,
            original_relpath: row.get(1)?,
            conflict_relpath: row.get(2)?,
            created_at_ms: row.get(3)?,
            reason: row.get(4)?,
        })
    })?;
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert_log(conn: &Connection, log: &LogRow) -> Result<()> {
    conn.execute(
        "INSERT INTO logs (task_id, level, event, detail, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![log.task_id, log.level, log.event, log.detail, log.created_at_ms],
    )?;
    Ok(())
}

pub fn list_logs(
    conn: &Connection,
    task_id: Option<&str>,
    level: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<LogRow>> {
    let mut sql = "SELECT task_id, level, event, detail, created_at_ms FROM logs".to_string();
    let mut filters = Vec::new();
    let mut params_vec: Vec<Value> = Vec::new();
    if task_id.is_some() {
        filters.push("task_id = ?1".to_string());
        params_vec.push(task_id.unwrap().to_string().into());
    }
    if level.is_some() {
        filters.push(if task_id.is_some() { "level = ?2" } else { "level = ?1" }.to_string());
        params_vec.push(level.unwrap().to_string().into());
    }
    if !filters.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&filters.join(" AND "));
    }
    sql.push_str(" ORDER BY created_at_ms DESC");
    if limit.is_some() {
        let idx = params_vec.len() + 1;
        sql.push_str(&format!(" LIMIT ?{}", idx));
        params_vec.push(Value::from(limit.unwrap() as i64));
        if offset.is_some() {
            let idx = params_vec.len() + 1;
            sql.push_str(&format!(" OFFSET ?{}", idx));
            params_vec.push(Value::from(offset.unwrap() as i64));
        }
    }

    let mut stmt = conn.prepare(&sql)?;
    let mut out = Vec::new();
    let rows = stmt.query_map(params_from_iter(params_vec), |row| {
        Ok(LogRow {
            task_id: row.get(0)?,
            level: row.get(1)?,
            event: row.get(2)?,
            detail: row.get(3)?,
            created_at_ms: row.get(4)?,
        })
    })?;
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count_logs(conn: &Connection, task_id: Option<&str>, level: Option<&str>) -> Result<u32> {
    let mut sql = "SELECT COUNT(1) FROM logs".to_string();
    let mut filters = Vec::new();
    let mut params_vec: Vec<Value> = Vec::new();
    if task_id.is_some() {
        filters.push("task_id = ?1".to_string());
        params_vec.push(task_id.unwrap().to_string().into());
    }
    if level.is_some() {
        filters.push(if task_id.is_some() { "level = ?2" } else { "level = ?1" }.to_string());
        params_vec.push(level.unwrap().to_string().into());
    }
    if !filters.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&filters.join(" AND "));
    }
    let mut stmt = conn.prepare(&sql)?;
    let count: u32 = stmt.query_row(params_from_iter(params_vec), |row| row.get(0))?;
    Ok(count)
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
