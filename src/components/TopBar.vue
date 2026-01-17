<template>
  <header class="top-bar">
    <div class="top-left">
      <div class="page-title">{{ title }}</div>
      <div class="page-subtitle">{{ subtitle }}</div>
    </div>
    <div class="top-actions">
      <el-input
        v-model="query"
        placeholder="搜索任务 / 文件 / 日志"
        class="search-input"
        size="large"
        @input="scheduleSearch"
        @focus="scheduleSearch"
      />
      <el-card v-if="showResults" class="search-results">
        <div class="search-section" v-if="taskResults.length">
          <div class="search-title">任务</div>
          <div class="search-item" v-for="task in taskResults" :key="task.id" @click="gotoTask(task)">
            <strong>{{ task.name }}</strong>
            <span>{{ task.local_path }} → {{ task.remote_path }}</span>
            <el-button size="small" text @click.stop="openTaskFolder(task)">打开目录</el-button>
          </div>
        </div>
        <div class="search-section" v-if="logResults.length">
          <div class="search-title">日志</div>
          <div class="search-item" v-for="log in logResults" :key="log.timestamp + log.detail" @click="gotoLogs">
            <strong>{{ log.event }}</strong>
            <span>{{ log.detail }}</span>
          </div>
        </div>
        <div class="search-section" v-if="conflictResults.length">
          <div class="search-title">冲突</div>
          <div class="search-item" v-for="conflict in conflictResults" :key="conflict.id" @click="gotoConflicts">
            <strong>{{ conflict.name }}</strong>
            <span>{{ conflict.path }}</span>
            <el-button size="small" text @click.stop="openConflictFolder(conflict)">打开目录</el-button>
            <el-button size="small" text @click.stop="downloadConflict(conflict)">下载云端</el-button>
          </div>
        </div>
        <div class="search-empty" v-if="!taskResults.length && !logResults.length && !conflictResults.length">
          暂无匹配结果
        </div>
      </el-card>
      <el-dropdown>
        <span class="account">
          账户：{{ activeAccountLabel }}
          <el-icon><arrow-down /></el-icon>
        </span>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item @click="gotoTasks">切换站点</el-dropdown-item>
            <el-dropdown-item @click="logout">退出登录</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ArrowDown } from "@element-plus/icons-vue";
import { useRouter } from "vue-router";
import { clearCredentials, listAccounts, listConflicts, listLogs, listTasks, openLocalPath, downloadConflictRemote } from "../services/api";
import type { AccountItem, ConflictItem, ActivityItem, TaskItem } from "../services/types";
import { ElMessage } from "element-plus";

defineProps<{ title: string; subtitle: string }>();

const query = ref("");
const accounts = ref<AccountItem[]>([]);
const taskResults = ref<TaskItem[]>([]);
const logResults = ref<ActivityItem[]>([]);
const conflictResults = ref<ConflictItem[]>([]);
const showResults = ref(false);
const router = useRouter();
let searchTimer: number | null = null;

const activeAccountLabel = computed(() => {
  if (!accounts.value.length) return "未登录";
  return accounts.value[0].email;
});

const loadAccounts = async () => {
  accounts.value = await listAccounts();
};

const scheduleSearch = () => {
  if (searchTimer) {
    window.clearTimeout(searchTimer);
  }
  searchTimer = window.setTimeout(async () => {
    const term = query.value.trim().toLowerCase();
    if (!term) {
      showResults.value = false;
      return;
    }
    const [tasks, logs, conflicts] = await Promise.all([
      listTasks(),
      listLogs({}),
      listConflicts()
    ]);
    taskResults.value = tasks.filter(item =>
      `${item.name} ${item.local_path} ${item.remote_path}`.toLowerCase().includes(term)
    ).slice(0, 6);
    logResults.value = logs.filter(item =>
      `${item.event} ${item.detail}`.toLowerCase().includes(term)
    ).slice(0, 6);
    conflictResults.value = conflicts.filter(item =>
      `${item.name} ${item.path}`.toLowerCase().includes(term)
    ).slice(0, 6);
    showResults.value = true;
  }, 300);
};

const gotoTask = async (task: TaskItem) => {
  showResults.value = false;
  await router.push("/tasks");
};

const gotoLogs = async () => {
  showResults.value = false;
  await router.push("/logs");
};

const gotoConflicts = async () => {
  showResults.value = false;
  await router.push("/conflicts");
};

const gotoTasks = async () => {
  await router.push("/tasks");
};

const logout = async () => {
  await clearCredentials();
  await loadAccounts();
  ElMessage.success("已退出登录");
};

const openTaskFolder = async (task: TaskItem) => {
  await openLocalPath(task.local_path);
};

const openConflictFolder = async (conflict: ConflictItem) => {
  await openLocalPath(conflict.local_dir);
};

const downloadConflict = async (conflict: ConflictItem) => {
  await downloadConflictRemote(conflict.task_id, conflict.original_relpath);
  ElMessage.success("已打开下载链接");
};

onMounted(loadAccounts);
</script>
