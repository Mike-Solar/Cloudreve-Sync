use rusqlite::Connection;
use tempfile::NamedTempFile;

use cloudreve_sync_app::core::db::{
    create_task, delete_task, init_db, insert_conflict, insert_log, insert_tombstone,
    list_accounts, list_conflicts, list_entries_by_task, list_logs, list_tasks, list_tombstones,
    now_ms, upsert_account, upsert_entry, AccountRow, ConflictRow, EntryRow, LogRow, TaskRow,
    TombstoneRow,
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

#[test]
fn db_filters_and_updates() {
    let file = NamedTempFile::new().expect("temp db");
    let conn = Connection::open(file.path()).expect("open db");
    init_db(&conn).expect("init db");

    let task_a = TaskRow {
        task_id: "task-a".to_string(),
        base_url: "https://example.com".to_string(),
        local_root: "/tmp/local-a".to_string(),
        remote_root_uri: "cloudreve://root/Work".to_string(),
        device_id: "device-a".to_string(),
        mode: "双向".to_string(),
        settings_json: "{}".to_string(),
        created_at_ms: now_ms(),
    };
    let task_b = TaskRow {
        task_id: "task-b".to_string(),
        base_url: "https://example.com".to_string(),
        local_root: "/tmp/local-b".to_string(),
        remote_root_uri: "cloudreve://root/Work".to_string(),
        device_id: "device-b".to_string(),
        mode: "双向".to_string(),
        settings_json: "{}".to_string(),
        created_at_ms: now_ms(),
    };
    create_task(&conn, &task_a).expect("create task a");
    create_task(&conn, &task_b).expect("create task b");
    assert_eq!(list_tasks(&conn).expect("list tasks").len(), 2);

    let account = AccountRow {
        account_key: "https://example.com|user@example.com".to_string(),
        base_url: "https://example.com".to_string(),
        email: "user@example.com".to_string(),
        created_at_ms: now_ms(),
    };
    upsert_account(&conn, &account).expect("upsert account");
    let accounts = list_accounts(&conn).expect("list accounts");
    assert_eq!(accounts.len(), 1);

    let entry_v1 = EntryRow {
        task_id: task_a.task_id.clone(),
        local_relpath: "doc.txt".to_string(),
        cloud_file_id: "file-1".to_string(),
        cloud_uri: "cloudreve://root/Work/doc.txt".to_string(),
        last_local_mtime_ms: 1,
        last_local_sha256: "a".to_string(),
        last_remote_mtime_ms: 1,
        last_remote_sha256: "a".to_string(),
        last_sync_ts_ms: 1,
        state: "ok".to_string(),
    };
    upsert_entry(&conn, &entry_v1).expect("upsert entry v1");
    let entry_v2 = EntryRow {
        last_local_mtime_ms: 2,
        last_local_sha256: "b".to_string(),
        last_remote_mtime_ms: 2,
        last_remote_sha256: "b".to_string(),
        last_sync_ts_ms: 2,
        ..entry_v1.clone()
    };
    upsert_entry(&conn, &entry_v2).expect("upsert entry v2");
    let entries = list_entries_by_task(&conn, &task_a.task_id).expect("list entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].last_local_sha256, "b");

    let tombstone_a = TombstoneRow {
        task_id: task_a.task_id.clone(),
        cloud_file_id: "file-a".to_string(),
        local_relpath: "a.txt".to_string(),
        deleted_at_ms: now_ms(),
        origin: "local".to_string(),
    };
    let tombstone_b = TombstoneRow {
        task_id: task_b.task_id.clone(),
        cloud_file_id: "file-b".to_string(),
        local_relpath: "b.txt".to_string(),
        deleted_at_ms: now_ms(),
        origin: "remote".to_string(),
    };
    insert_tombstone(&conn, &tombstone_a).expect("insert tombstone a");
    insert_tombstone(&conn, &tombstone_b).expect("insert tombstone b");
    let tombstones_a = list_tombstones(&conn, &task_a.task_id).expect("list tombstones a");
    let tombstones_b = list_tombstones(&conn, &task_b.task_id).expect("list tombstones b");
    assert_eq!(tombstones_a.len(), 1);
    assert_eq!(tombstones_b.len(), 1);

    let conflict_a = ConflictRow {
        task_id: task_a.task_id.clone(),
        original_relpath: "doc.txt".to_string(),
        conflict_relpath: "doc (conflict).txt".to_string(),
        created_at_ms: now_ms(),
        reason: "both_modified".to_string(),
    };
    let conflict_b = ConflictRow {
        task_id: task_b.task_id.clone(),
        original_relpath: "photo.jpg".to_string(),
        conflict_relpath: "photo (conflict).jpg".to_string(),
        created_at_ms: now_ms(),
        reason: "both_modified".to_string(),
    };
    insert_conflict(&conn, &conflict_a).expect("insert conflict a");
    insert_conflict(&conn, &conflict_b).expect("insert conflict b");
    assert_eq!(
        list_conflicts(&conn, None)
            .expect("list conflicts all")
            .len(),
        2
    );
    assert_eq!(
        list_conflicts(&conn, Some(&task_a.task_id))
            .expect("list conflicts a")
            .len(),
        1
    );

    let log_info = LogRow {
        task_id: task_a.task_id.clone(),
        level: "info".to_string(),
        event: "upload".to_string(),
        detail: "doc.txt".to_string(),
        created_at_ms: now_ms(),
    };
    let log_warn = LogRow {
        task_id: task_a.task_id.clone(),
        level: "warn".to_string(),
        event: "delete".to_string(),
        detail: "old.txt".to_string(),
        created_at_ms: now_ms(),
    };
    insert_log(&conn, &log_info).expect("insert log info");
    insert_log(&conn, &log_warn).expect("insert log warn");
    assert_eq!(
        list_logs(&conn, Some(&task_a.task_id), Some("info"))
            .expect("list logs info")
            .len(),
        1
    );
    assert_eq!(
        list_logs(&conn, Some(&task_a.task_id), Some("warn"))
            .expect("list logs warn")
            .len(),
        1
    );
}

#[test]
fn delete_task_removes_related_rows() {
    let file = NamedTempFile::new().expect("temp db");
    let conn = Connection::open(file.path()).expect("open db");
    init_db(&conn).expect("init db");

    let task = TaskRow {
        task_id: "task-delete".to_string(),
        base_url: "https://example.com".to_string(),
        local_root: "/tmp/local-delete".to_string(),
        remote_root_uri: "cloudreve://root/Work".to_string(),
        device_id: "device-delete".to_string(),
        mode: "双向".to_string(),
        settings_json: "{}".to_string(),
        created_at_ms: now_ms(),
    };
    create_task(&conn, &task).expect("create task");

    let entry = EntryRow {
        task_id: task.task_id.clone(),
        local_relpath: "doc.txt".to_string(),
        cloud_file_id: "file-1".to_string(),
        cloud_uri: "cloudreve://root/Work/doc.txt".to_string(),
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

    let log = LogRow {
        task_id: task.task_id.clone(),
        level: "info".to_string(),
        event: "upload".to_string(),
        detail: "doc.txt".to_string(),
        created_at_ms: now_ms(),
    };
    insert_log(&conn, &log).expect("insert log");

    delete_task(&conn, &task.task_id).expect("delete task");
    assert!(list_tasks(&conn).expect("list tasks").is_empty());
    assert!(list_entries_by_task(&conn, &task.task_id)
        .expect("list entries")
        .is_empty());
    assert!(list_tombstones(&conn, &task.task_id)
        .expect("list tombstones")
        .is_empty());
    assert!(list_conflicts(&conn, Some(&task.task_id))
        .expect("list conflicts")
        .is_empty());
    assert!(list_logs(&conn, Some(&task.task_id), None)
        .expect("list logs")
        .is_empty());
}
