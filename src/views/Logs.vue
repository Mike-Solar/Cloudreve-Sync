<template>
  <section class="logs-view">
    <el-card class="panel">
      <div class="panel-header">
        <div>
          <div class="panel-title">活动日志</div>
          <div class="panel-subtitle">任务级别的可追溯记录</div>
        </div>
        <div class="log-actions">
          <el-button @click="refresh">刷新</el-button>
          <el-button type="primary" plain @click="exportLogFile">导出日志</el-button>
          <el-switch v-model="autoRefresh" active-text="实时刷新" />
        </div>
      </div>
      <div class="log-filters">
        <el-input v-model="taskId" placeholder="任务 ID（可选）" />
        <el-select v-model="level" placeholder="日志级别">
          <el-option label="info" value="info" />
          <el-option label="warn" value="warn" />
          <el-option label="error" value="error" />
        </el-select>
        <el-input v-model="search" placeholder="文件名 / 路径 / 错误码" />
      </div>
      <el-table :data="filtered" class="table-flat">
        <el-table-column prop="timestamp" label="时间" width="160" />
        <el-table-column prop="event" label="类型" width="120" />
        <el-table-column prop="detail" label="详情" />
        <el-table-column prop="level" label="级别" width="100" />
      </el-table>
    </el-card>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ActivityItem } from "../services/types";
import { exportLogs, listLogs, openLocalPath } from "../services/api";
import { ElMessage } from "element-plus";

const logs = ref<ActivityItem[]>([]);
const search = ref("");
const level = ref("");
const taskId = ref("");
let refreshTimer: number | null = null;
const autoRefresh = ref(true);

const refresh = async () => {
  logs.value = await listLogs({
    task_id: taskId.value || undefined,
    level: level.value || undefined
  });
};

watch([level, taskId], refresh);

const filtered = computed(() => {
  return logs.value.filter(item => {
    if (search.value) {
      const term = search.value.toLowerCase();
      return `${item.detail} ${item.event}`.toLowerCase().includes(term);
    }
    return true;
  });
});

const startAutoRefresh = () => {
  if (refreshTimer) {
    window.clearInterval(refreshTimer);
  }
  refreshTimer = window.setInterval(refresh, 1000);
};

const stopAutoRefresh = () => {
  if (refreshTimer) {
    window.clearInterval(refreshTimer);
    refreshTimer = null;
  }
};

const pauseAutoRefresh = () => {
  if (autoRefresh.value) {
    stopAutoRefresh();
  }
};

const resumeAutoRefresh = () => {
  if (autoRefresh.value) {
    startAutoRefresh();
  }
};

onMounted(() => {
  refresh();
  if (autoRefresh.value) {
    startAutoRefresh();
  }
});

watch(autoRefresh, value => {
  if (value) {
    startAutoRefresh();
  } else {
    stopAutoRefresh();
  }
});

onBeforeUnmount(() => {
  stopAutoRefresh();
});

const exportLogFile = async () => {
  const path = await exportLogs({
    task_id: taskId.value || undefined,
    level: level.value || undefined
  });
  await openLocalPath(path);
  ElMessage.success("日志已导出");
};

watch(search, value => {
  if (value) {
    pauseAutoRefresh();
  } else {
    resumeAutoRefresh();
  }
});

watch(taskId, () => {
  pauseAutoRefresh();
  refresh().finally(resumeAutoRefresh);
});

watch(level, () => {
  pauseAutoRefresh();
  refresh().finally(resumeAutoRefresh);
});
</script>
