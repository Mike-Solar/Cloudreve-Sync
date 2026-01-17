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
  name: string;
  task: string;
  path: string;
  device: string;
  time: string;
  status: string;
}

export interface BootstrapPayload {
  cards: DashboardCard[];
  tasks: TaskItem[];
  activities: ActivityItem[];
  conflicts: ConflictItem[];
}
