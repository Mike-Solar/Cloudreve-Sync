export interface DashboardCard {
  label: string;
  value: string;
  tone: string;
}

export interface TaskItem {
  id: string;
  name: string;
  mode: string;
  local_path: string;
  remote_path: string;
  status: string;
  rate_up: string;
  rate_down: string;
  queue: number;
  last_sync: string;
}

export interface ActivityItem {
  timestamp: string;
  event: string;
  detail: string;
  level: string;
}

export interface ConflictItem {
  id: string;
  task_id: string;
  original_relpath: string;
  conflict_relpath: string;
  name: string;
  task: string;
  path: string;
  local_path: string;
  local_dir: string;
  device: string;
  time: string;
  status: string;
}

export interface AccountItem {
  account_key: string;
  base_url: string;
  email: string;
  created_at_ms: number;
}

export interface AppSettings {
  autostart: boolean;
  tray: boolean;
  language: string;
  proxy: string;
  retries: number;
  backoff: string;
  upload: number;
  download: number;
  sha_threads: number;
  lock_pause: boolean;
  debug: boolean;
  trace: boolean;
}

export interface DiagnosticInfo {
  app_version: string;
  os: string;
  arch: string;
  db_path: string;
  config_dir: string;
  accounts: number;
  tasks: number;
}

export interface BootstrapPayload {
  cards: DashboardCard[];
  tasks: TaskItem[];
  activities: ActivityItem[];
  conflicts: ConflictItem[];
}
