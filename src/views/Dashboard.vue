<template>
  <section class="dashboard">
    <div class="card-grid">
      <el-card v-for="card in cards" :key="card.label" class="metric-card">
        <div class="metric-label">{{ card.label }}</div>
        <div class="metric-value" :data-tone="card.tone">{{ card.value }}</div>
      </el-card>
    </div>

    <div class="panel-grid">
      <el-card class="panel">
        <div class="panel-header">
          <div>
            <div class="panel-title">{{ t("dashboard.currentTasks") }}</div>
            <div class="panel-subtitle">{{ t("dashboard.currentTasksSub") }}</div>
          </div>
          <el-button type="primary" plain @click="gotoTasks">{{ t("dashboard.viewAll") }}</el-button>
        </div>
        <div class="task-list">
          <div v-for="task in tasks" :key="task.id" class="task-row">
            <div>
              <div class="task-name">{{ task.name }}</div>
              <div class="task-path">{{ task.local_path }} → {{ task.remote_path }}</div>
            </div>
            <div class="task-meta">
              <el-tag :type="statusTone(task.status)" effect="dark">{{ localizedStatus(task.status) }}</el-tag>
              <div class="task-queue">{{ task.progress_text }}</div>
              <div class="task-rate">↑ {{ task.rate_up }} ↓ {{ task.rate_down }}</div>
              <div class="task-queue">{{ t("dashboard.queue") }} {{ task.queue }}</div>
            </div>
            <div class="task-actions">
              <el-button size="small" @click="toggleSync(task)">
                {{ isRunningStatus(task.status) ? t("dashboard.pause") : t("dashboard.sync") }}
              </el-button>
              <el-button size="small" type="primary" @click="openTaskFolder(task)">
                {{ t("dashboard.openFolder") }}
              </el-button>
            </div>
          </div>
        </div>
      </el-card>

      <el-card class="panel">
        <div class="panel-header">
          <div>
            <div class="panel-title">{{ t("dashboard.recent") }}</div>
            <div class="panel-subtitle">{{ t("dashboard.recentSub") }}</div>
          </div>
          <el-button type="primary" plain @click="gotoLogs">{{ t("dashboard.viewLogs") }}</el-button>
        </div>
        <el-timeline class="activity-timeline">
          <el-timeline-item
            v-for="activity in activities.slice(0, 10)"
            :key="activity.timestamp + activity.detail"
            :type="activityTone(activity.level)"
            :timestamp="activity.timestamp"
          >
            <div class="activity-item">
              <strong>{{ activity.event }}</strong>
              <span>{{ activity.detail }}</span>
            </div>
          </el-timeline-item>
        </el-timeline>
      </el-card>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import type { ActivityItem, DashboardCard, TaskItem, TaskRuntimePayload } from "../services/types";
import { fetchBootstrap } from "../services/bootstrap";
import { openLocalPath, runSync, stopSync } from "../services/api";

const cards = ref<DashboardCard[]>([]);
const tasks = ref<TaskItem[]>([]);
const activities = ref<ActivityItem[]>([]);
const router = useRouter();
const { t } = useI18n();
let unlistenTaskRuntime: UnlistenFn | null = null;
const isRunningStatus = (status: string) => ["Syncing", "Hashing", "ListingRemote"].includes(status);

const localizedStatus = (status: string) => {
  if (status === "Syncing") return t("common.statusSyncing");
  if (status === "Hashing") return t("common.statusHashing");
  if (status === "ListingRemote") return t("common.statusListingRemote");
  if (status === "Paused") return t("common.statusPaused");
  if (status === "Error") return t("common.statusError");
  if (status === "Conflict") return t("common.statusConflict");
  return status;
};

const localizedCard = (card: DashboardCard): DashboardCard => {
  let label = card.label;
  let value = card.value;
  if (card.label === "同步状态") {
    label = t("dashboard.cardSyncState");
    value = card.value === "运行中" ? t("dashboard.running") : t("dashboard.paused");
  } else if (card.label === "今日上传") {
    label = t("dashboard.cardUploadToday");
    if (card.value.endsWith(" 文件")) {
      value = `${card.value.replace(" 文件", "")} ${t("dashboard.filesSuffix")}`;
    }
  } else if (card.label === "今日下载") {
    label = t("dashboard.cardDownloadToday");
    if (card.value.endsWith(" 文件")) {
      value = `${card.value.replace(" 文件", "")} ${t("dashboard.filesSuffix")}`;
    }
  } else if (card.label === "未处理冲突") {
    label = t("dashboard.cardConflicts");
  }
  return { ...card, label, value };
};

const applyTaskRuntime = (payload: TaskRuntimePayload) => {
  const index = tasks.value.findIndex(item => item.id === payload.task_id);
  if (index >= 0) {
    const current = tasks.value[index];
    tasks.value[index] = {
      ...current,
      status: payload.status || current.status,
      progress_text: payload.progress_text || current.progress_text,
      rate_up: payload.rate_up,
      rate_down: payload.rate_down,
      queue: payload.queue,
      last_sync: payload.last_sync || current.last_sync
    };
  }
  const syncing = tasks.value.some(item => isRunningStatus(item.status));
  const syncCard = cards.value.find(item => item.tone === "success" || item.tone === "warn");
  if (syncCard) {
    syncCard.label = t("dashboard.cardSyncState");
    syncCard.value = syncing ? t("dashboard.running") : t("dashboard.paused");
    syncCard.tone = syncing ? "success" : "warn";
  }
};

onMounted(async () => {
  const data = await fetchBootstrap();
  cards.value = data.cards.map(localizedCard);
  tasks.value = data.tasks;
  activities.value = data.activities;
  unlistenTaskRuntime = await listen<TaskRuntimePayload>("task-runtime", event => {
    applyTaskRuntime(event.payload);
  });
});

onBeforeUnmount(() => {
  if (unlistenTaskRuntime) {
    unlistenTaskRuntime();
    unlistenTaskRuntime = null;
  }
});

const statusTone = (status: string) => {
  if (isRunningStatus(status)) return "success";
  if (status === "Error") return "danger";
  if (status === "Paused") return "warning";
  return "info";
};

const activityTone = (level: string) => {
  if (level === "warn") return "warning";
  if (level === "error") return "danger";
  return "success";
};

const gotoTasks = () => {
  router.push("/tasks");
};

const gotoLogs = () => {
  router.push("/logs");
};

const toggleSync = async (task: TaskItem) => {
  if (isRunningStatus(task.status)) {
    await stopSync({ task_id: task.id });
  } else {
    await runSync({ task_id: task.id });
  }
  const data = await fetchBootstrap();
  tasks.value = data.tasks;
};

const openTaskFolder = async (task: TaskItem) => {
  await openLocalPath(task.local_path);
};
</script>
