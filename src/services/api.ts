import { invoke } from "@tauri-apps/api/tauri";
import type { BootstrapPayload, ConflictItem, TaskItem, ActivityItem } from "./types";

export interface LoginRequest {
  base_url: string;
  email: string;
  password: string;
  captcha?: string;
  ticket?: string;
}

export interface LoginResult {
  account_key: string;
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

export async function login(payload: LoginRequest): Promise<LoginResult> {
  return invoke("login", payload);
}

export async function getCaptcha(baseUrl: string) {
  return invoke("get_captcha_command", baseUrl);
}

export async function testConnection(account_key: string, base_url: string) {
  return invoke("test_connection", { account_key, base_url });
}

export async function createTask(payload: CreateTaskRequest) {
  return invoke("create_task_command", payload);
}

export async function listTasks(): Promise<TaskItem[]> {
  return invoke("list_tasks_command");
}

export async function listConflicts(task_id?: string): Promise<ConflictItem[]> {
  return invoke("list_conflicts_command", { task_id });
}

export async function listLogs(query: LogsQuery): Promise<ActivityItem[]> {
  return invoke("list_logs_command", { query });
}

export async function runSync(payload: SyncRequest) {
  return invoke("run_sync_command", payload);
}

export async function stopSync(payload: SyncRequest) {
  return invoke("stop_sync_command", payload);
}

export async function fetchBootstrap(): Promise<BootstrapPayload> {
  return invoke("bootstrap");
}
