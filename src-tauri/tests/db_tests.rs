use rusqlite::Connection;
use tempfile::NamedTempFile;

use cloudreve_sync_app::core::db::{
    create_task, init_db, insert_conflict, insert_log, insert_tombstone, list_conflicts, list_logs,
    list_tasks, now_ms, upsert_entry, ConflictRow, EntryRow, LogRow, TaskRow, TombstoneRow,
};

#[test]
fn db_init_and_crud() {
    let file = NamedTempFile::new().expect("temp db");
    let conn = Connection::open(file.path()).expect("open db");
    init_db(&conn).expect("init db");

    let task = TaskRow {
        task_id: "task-1".to_string(),
        base_url: "https://example.com".to_string(),
        local_root: "/tmp/local".to_string(),
        remote_root_uri: "cloudreve://my/Work".to_string(),
        device_id: "device-1".to_string(),
        mode: "双向".to_string(),
        settings_json: "{}".to_string(),
        created_at_ms: now_ms(),
    };
    create_task(&conn, &task).expect("create task");
    let tasks = list_tasks(&conn).expect("list tasks");
    assert_eq!(tasks.len(), 1);

    let entry = EntryRow {
        task_id: task.task_id.clone(),
        local_relpath: "doc.txt".to_string(),
        cloud_file_id: "file-1".to_string(),
        cloud_uri: "cloudreve://my/Work/doc.txt".to_string(),
        last_local_mtime_ms: 1,
        last_local_sha256: "a".to_string(),
        last_remote_mtime_ms: 1,
        last_remote_sha256: "a".to_string(),
        last_sync_ts_ms: 1,
        state: "ok".to_string(),
    };
    upsert_entry(&conn, &entry).expect("upsert entry");

    let tombstone = TombstoneRow {
        task_id: task.task_id.clone(),
        cloud_file_id: "file-1".to_string(),
        local_relpath: "doc.txt".to_string(),
        deleted_at_ms: now_ms(),
        origin: "local".to_string(),
    };
    insert_tombstone(&conn, &tombstone).expect("insert tombstone");

    let conflict = ConflictRow {
        task_id: task.task_id.clone(),
        original_relpath: "doc.txt".to_string(),
        conflict_relpath: "doc (conflict).txt".to_string(),
        created_at_ms: now_ms(),
        reason: "both_modified".to_string(),
    };
    insert_conflict(&conn, &conflict).expect("insert conflict");
    let conflicts = list_conflicts(&conn, Some(&task.task_id)).expect("list conflicts");
    assert_eq!(conflicts.len(), 1);

    let log = LogRow {
        task_id: task.task_id.clone(),
        level: "info".to_string(),
        event: "upload".to_string(),
        detail: "doc.txt".to_string(),
        created_at_ms: now_ms(),
    };
    insert_log(&conn, &log).expect("insert log");
    let logs = list_logs(&conn, Some(&task.task_id), None).expect("list logs");
    assert_eq!(logs.len(), 1);
}
