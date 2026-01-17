import { invoke } from "@tauri-apps/api/core";
import type {
  BootstrapPayload,
  ConflictItem,
  TaskItem,
  ActivityItem,
  AccountItem,
  AppSettings,
  DiagnosticInfo,
  RemoteEntry
} from "./types";

export interface LoginRequest {
  base_url: string;
  email: string;
  password: string;
  captcha?: string;
  ticket?: string;
}

export type LoginResult =
  | { status: "success"; account_key: string }
  | { status: "two_fa_required"; session_id: string };

export interface TwoFaFinishRequest {
  base_url: string;
  email: string;
  session_id: string;
  opt: string;
}

export interface CreateTaskRequest {
  name: string;
  base_url: string;
  account_key: string;
  local_root: string;
  remote_root_uri: string;
  mode: string;
  sync_interval_secs: number;
}

export interface LogsQuery {
  task_id?: string;
  level?: string;
}

export interface SyncRequest {
  task_id: string;
}

export interface DeleteTaskRequest {
  task_id: string;
}

export interface ListRemoteEntriesRequest {
  account_key: string;
  base_url: string;
  uri: string;
}

export async function login(payload: LoginRequest): Promise<LoginResult> {
  return invoke("login", { payload });
}

export async function finishSignInWith2fa(payload: TwoFaFinishRequest): Promise<LoginResult> {
  return invoke("finish_sign_in_with_2fa_command", { payload });
}

export async function getCaptcha(baseUrl: string) {
  return invoke("get_captcha_command", { payload: baseUrl });
}

export async function testConnection(account_key: string, base_url: string) {
  return invoke("test_connection", { accountKey: account_key, baseUrl: base_url });
}

export async function createTask(payload: CreateTaskRequest) {
  return invoke("create_task_command", { payload });
}

export async function listTasks(): Promise<TaskItem[]> {
  return invoke("list_tasks_command");
}

export async function listAccounts(): Promise<AccountItem[]> {
  return invoke("list_accounts_command");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings_command");
}

export async function saveSettings(payload: AppSettings) {
  return invoke("save_settings_command", { payload });
}

export async function clearCredentials() {
  return invoke("clear_credentials_command");
}

export async function openLocalPath(path: string) {
  return invoke("open_local_path", { path });
}

export async function listConflicts(task_id?: string): Promise<ConflictItem[]> {
  return invoke("list_conflicts_command", { task_id });
}

export async function listRemoteEntries(payload: ListRemoteEntriesRequest): Promise<RemoteEntry[]> {
  return invoke("list_remote_entries_command", { payload });
}

export async function markConflictResolved(task_id: string, conflict_relpath: string) {
  return invoke("mark_conflict_resolved", { task_id, conflict_relpath });
}

export async function downloadConflictRemote(task_id: string, original_relpath: string) {
  return invoke("download_conflict_remote", { task_id, original_relpath });
}

export async function hashLocalFile(path: string): Promise<string> {
  return invoke("hash_local_file", { path });
}

export async function openExternal(url: string) {
  return invoke("open_external", { url });
}

export async function getDiagnostics(): Promise<DiagnosticInfo> {
  return invoke("get_diagnostics_command");
}

export async function exportLogs(query: LogsQuery): Promise<string> {
  return invoke("export_logs_command", {
    task_id: query.task_id,
    level: query.level
  });
}

export async function listLogs(query: LogsQuery): Promise<ActivityItem[]> {
  return invoke("list_logs_command", { query });
}

export async function runSync(payload: SyncRequest) {
  return invoke("run_sync_command", { payload });
}

export async function stopSync(payload: SyncRequest) {
  return invoke("stop_sync_command", { payload });
}

export async function deleteTask(payload: DeleteTaskRequest) {
  return invoke("delete_task_command", { payload });
}

export async function fetchBootstrap(): Promise<BootstrapPayload> {
  return invoke("bootstrap");
}
