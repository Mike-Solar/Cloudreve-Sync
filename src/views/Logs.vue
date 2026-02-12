<template>
  <section class="logs-view">
    <el-card class="panel">
      <div class="panel-header">
        <div>
          <div class="panel-title">{{ t("logs.title") }}</div>
          <div class="panel-subtitle">{{ t("logs.sub") }}</div>
        </div>
        <div class="log-actions">
          <el-button @click="refresh">{{ t("logs.refresh") }}</el-button>
          <el-button type="primary" plain @click="exportLogFile">{{ t("logs.export") }}</el-button>
          <el-switch v-model="autoRefresh" :active-text="t('logs.autoRefresh')" />
        </div>
      </div>
      <div class="log-filters">
        <el-input v-model="taskId" :placeholder="t('logs.taskIdPlaceholder')" />
        <el-select v-model="level" :placeholder="t('logs.levelPlaceholder')">
          <el-option label="info" value="info" />
          <el-option label="warn" value="warn" />
          <el-option label="error" value="error" />
        </el-select>
        <el-input v-model="search" :placeholder="t('logs.searchPlaceholder')" />
      </div>
      <div class="log-table" @scroll="handleTableScroll">
        <el-table :data="filtered" class="table-flat" height="100%">
          <el-table-column prop="timestamp" :label="t('logs.colTime')" width="160" />
          <el-table-column prop="event" :label="t('logs.colType')" width="120" />
          <el-table-column prop="detail" :label="t('logs.colDetail')" />
          <el-table-column prop="level" :label="t('logs.colLevel')" width="100" />
        </el-table>
      </div>
      <div class="log-pagination">
        <el-pagination
          background
          layout="total, sizes, prev, pager, next, jumper"
          :total="total"
          :page-size="pageSize"
          :current-page="page"
          :page-sizes="[10, 20, 50, 100, 200]"
          @update:current-page="handlePageChange"
          @update:page-size="handlePageSizeChange"
        />
      </div>
    </el-card>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ActivityItem, LogsPage } from "../services/types";
import { exportLogs, listLogs, openLocalPath } from "../services/api";
import { ElMessage } from "element-plus";
import { useI18n } from "vue-i18n";

const logs = ref<ActivityItem[]>([]);
const search = ref("");
const level = ref("");
const taskId = ref("");
const page = ref(1);
const pageSize = ref(50);
const total = ref(0);
let refreshTimer: number | null = null;
const autoRefresh = ref(true);
let scrollResumeTimer: number | null = null;
const { t } = useI18n();

const refresh = async () => {
  const result: LogsPage = await listLogs({
    task_id: taskId.value || undefined,
    level: level.value || undefined,
    page: page.value,
    page_size: pageSize.value
  });
  logs.value = result.items;
  total.value = result.total;
};

watch([level, taskId], () => {
  page.value = 1;
  refresh();
});

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

const handleTableScroll = () => {
  pauseAutoRefresh();
  if (scrollResumeTimer) {
    window.clearTimeout(scrollResumeTimer);
  }
  scrollResumeTimer = window.setTimeout(() => {
    scrollResumeTimer = null;
    resumeAutoRefresh();
  }, 1500);
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
  if (scrollResumeTimer) {
    window.clearTimeout(scrollResumeTimer);
    scrollResumeTimer = null;
  }
});

const exportLogFile = async () => {
  const path = await exportLogs({
    task_id: taskId.value || undefined,
    level: level.value || undefined
  });
  await openLocalPath(path);
  ElMessage.success(t("logs.exported"));
};

watch(search, value => {
  if (value) {
    pauseAutoRefresh();
  } else {
    resumeAutoRefresh();
  }
});

const handlePageChange = (value: number) => {
  page.value = value;
  pauseAutoRefresh();
  refresh().finally(resumeAutoRefresh);
};

const handlePageSizeChange = (value: number) => {
  pageSize.value = value;
  page.value = 1;
  pauseAutoRefresh();
  refresh().finally(resumeAutoRefresh);
};
</script>

<style scoped>
.logs-view {
  height: 100%;
}

.panel {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 180px);
}

.log-table {
  flex: 1;
  min-height: 240px;
  overflow: auto;
}

.log-pagination {
  display: flex;
  justify-content: flex-end;
  padding-top: 12px;
}

.log-table :deep(.el-table__body-wrapper) {
  overflow: visible;
}
</style>
