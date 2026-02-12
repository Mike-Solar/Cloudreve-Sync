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
            <div class="panel-title">当前任务</div>
            <div class="panel-subtitle">最近活动的同步任务</div>
          </div>
          <el-button type="primary" plain @click="gotoTasks">查看全部</el-button>
        </div>
        <div class="task-list">
          <div v-for="task in tasks" :key="task.id" class="task-row">
            <div>
              <div class="task-name">{{ task.name }}</div>
              <div class="task-path">{{ task.local_path }} → {{ task.remote_path }}</div>
            </div>
            <div class="task-meta">
              <el-tag :type="statusTone(task.status)" effect="dark">{{ task.status }}</el-tag>
              <div class="task-queue">{{ task.progress_text }}</div>
              <div class="task-rate">↑ {{ task.rate_up }} ↓ {{ task.rate_down }}</div>
              <div class="task-queue">队列 {{ task.queue }}</div>
            </div>
            <div class="task-actions">
              <el-button size="small" @click="toggleSync(task)">
                {{ isRunningStatus(task.status) ? "暂停" : "同步" }}
              </el-button>
              <el-button size="small" type="primary" @click="openTaskFolder(task)">打开目录</el-button>
            </div>
          </div>
        </div>
      </el-card>

      <el-card class="panel">
        <div class="panel-header">
          <div>
            <div class="panel-title">最近活动</div>
            <div class="panel-subtitle">上传 / 下载 / 冲突 / 删除</div>
          </div>
          <el-button type="primary" plain @click="gotoLogs">查看日志</el-button>
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
import type { ActivityItem, DashboardCard, TaskItem, TaskRuntimePayload } from "../services/types";
import { fetchBootstrap } from "../services/bootstrap";
import { openLocalPath, runSync, stopSync } from "../services/api";

const cards = ref<DashboardCard[]>([]);
const tasks = ref<TaskItem[]>([]);
const activities = ref<ActivityItem[]>([]);
const router = useRouter();
let unlistenTaskRuntime: UnlistenFn | null = null;
const isRunningStatus = (status: string) => ["Syncing", "Hashing", "ListingRemote"].includes(status);

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
  const syncCard = cards.value.find(item => item.label === "同步状态");
  if (syncCard) {
    syncCard.value = syncing ? "运行中" : "已暂停";
    syncCard.tone = syncing ? "success" : "warn";
  }
};

onMounted(async () => {
  const data = await fetchBootstrap();
  cards.value = data.cards;
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
