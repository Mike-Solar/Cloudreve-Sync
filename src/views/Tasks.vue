<template>
  <section class="tasks-view">
    <div class="toolbar">
      <el-button type="primary" @click="wizardVisible = true">+ 新建任务</el-button>
      <div class="toolbar-actions">
        <el-button @click="refresh">刷新</el-button>
      </div>
      <div class="toolbar-filters">
        <el-checkbox v-model="onlyErrors">仅错误</el-checkbox>
        <el-checkbox v-model="onlyConflicts">仅冲突</el-checkbox>
        <el-checkbox v-model="recent">最近活跃</el-checkbox>
      </div>
    </div>

    <el-table :data="filtered" class="table-flat">
      <el-table-column prop="name" label="任务名" width="160" />
      <el-table-column prop="mode" label="模式" width="100" />
      <el-table-column prop="local_path" label="本地目录" />
      <el-table-column prop="remote_path" label="云端目录" />
      <el-table-column label="状态" width="140">
        <template #default="{ row }">
          <el-tag :type="statusTone(row.status)" effect="dark">{{ row.status }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="220">
        <template #default="{ row }">
          <el-button size="small" @click="toggleSync(row)">
            {{ row.status === "Syncing" ? "暂停" : "同步" }}
          </el-button>
          <el-button size="small" plain @click="refresh">刷新</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="wizardVisible" title="新建同步任务" width="720px">
      <el-steps :active="step" finish-status="success" align-center>
        <el-step title="账号与站点" />
        <el-step title="选择目录" />
        <el-step title="策略确认" />
        <el-step title="首次同步" />
      </el-steps>

      <div class="wizard-body" v-if="step === 0">
        <el-input v-model="wizard.base_url" placeholder="Cloudreve Base URL" />
        <el-input v-model="wizard.email" placeholder="邮箱" />
        <el-input v-model="wizard.password" placeholder="密码" type="password" show-password />
        <el-input v-model="wizard.captcha" placeholder="验证码（如需要）" />
        <el-input v-model="wizard.ticket" placeholder="Captcha Ticket（如需要）" />
        <el-button type="primary" @click="doLogin">登录并测试连接</el-button>
        <el-button plain @click="fetchCaptcha">获取验证码</el-button>
        <div v-if="captchaImage" class="captcha-preview">
          <img :src="captchaImage" alt="captcha" />
        </div>
      </div>

      <div class="wizard-body" v-else-if="step === 1">
        <el-input v-model="wizard.task_name" placeholder="任务名称" />
        <el-input v-model="wizard.local_root" placeholder="本地目录" />
        <el-input v-model="wizard.remote_root_uri" placeholder="云端目录 (URI 或路径)" />
      </div>

      <div class="wizard-body" v-else-if="step === 2">
        <el-radio-group v-model="wizard.mode">
          <el-radio label="双向">双向同步（默认）</el-radio>
          <el-radio label="单向→">本地 → 云端</el-radio>
          <el-radio label="单向←">云端 → 本地</el-radio>
        </el-radio-group>
        <el-alert type="info" show-icon title="冲突双保留与软删除策略不可修改" />
      </div>

      <div class="wizard-body" v-else>
        <el-radio-group v-model="wizard.first_sync">
          <el-radio label="sync">立即同步</el-radio>
          <el-radio label="index">仅建立索引</el-radio>
        </el-radio-group>
        <el-input-number v-model="wizard.sync_interval_secs" :min="5" label="同步间隔 (秒)" />
      </div>

      <template #footer>
        <div class="wizard-footer">
          <el-button @click="wizardVisible = false">取消</el-button>
          <el-button :disabled="step === 0" @click="step--">上一步</el-button>
          <el-button v-if="step < 3" type="primary" @click="step++">下一步</el-button>
          <el-button v-else type="primary" @click="submitTask">创建任务</el-button>
        </div>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import type { TaskItem } from "../services/types";
import {
  createTask,
  fetchBootstrap,
  listTasks,
  login,
  runSync,
  stopSync,
  testConnection,
  getCaptcha
} from "../services/api";

const tasks = ref<TaskItem[]>([]);
const onlyErrors = ref(false);
const onlyConflicts = ref(false);
const recent = ref(true);
const wizardVisible = ref(false);
const step = ref(0);
const captchaImage = ref<string | null>(null);

const wizard = ref({
  base_url: "",
  email: "",
  password: "",
  captcha: "",
  ticket: "",
  account_key: "",
  task_name: "",
  local_root: "",
  remote_root_uri: "",
  mode: "双向",
  first_sync: "sync",
  sync_interval_secs: 60
});

const refresh = async () => {
  tasks.value = await listTasks();
};

const filtered = computed(() => {
  return tasks.value.filter(item => {
    if (onlyErrors.value && item.status !== "Error") return false;
    if (onlyConflicts.value && item.status !== "Conflict") return false;
    if (recent.value && item.last_sync === "--") return false;
    return true;
  });
});

const statusTone = (status: string) => {
  if (status === "Syncing") return "success";
  if (status === "Error") return "danger";
  if (status === "Paused") return "warning";
  return "info";
};

const doLogin = async () => {
  const result = await login({
    base_url: wizard.value.base_url,
    email: wizard.value.email,
    password: wizard.value.password,
    captcha: wizard.value.captcha || undefined,
    ticket: wizard.value.ticket || undefined
  });
  await testConnection(result.account_key, wizard.value.base_url);
  wizard.value.account_key = result.account_key;
  ElMessage.success("连接成功");
};

const fetchCaptcha = async () => {
  if (!wizard.value.base_url) {
    ElMessage.warning("请先填写 Base URL");
    return;
  }
  const data: any = await getCaptcha(wizard.value.base_url);
  wizard.value.ticket = data.ticket;
  captchaImage.value = data.image;
};

const submitTask = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error("请先登录并验证连接");
    return;
  }
  await createTask({
    name: wizard.value.task_name || "新任务",
    base_url: wizard.value.base_url,
    account_key: wizard.value.account_key,
    local_root: wizard.value.local_root,
    remote_root_uri: wizard.value.remote_root_uri,
    mode: wizard.value.mode,
    sync_interval_secs: wizard.value.sync_interval_secs
  });
  wizardVisible.value = false;
  step.value = 0;
  await refresh();
  if (wizard.value.first_sync === "sync") {
    const created = tasks.value.find(item => item.name === (wizard.value.task_name || "新任务"));
    if (created) {
      await runSync({ task_id: created.id });
      await refresh();
    }
  }
  ElMessage.success("任务已创建");
};

const toggleSync = async (row: TaskItem) => {
  if (row.status === "Syncing") {
    await stopSync({ task_id: row.id });
  } else {
    await runSync({ task_id: row.id });
  }
  await refresh();
};

onMounted(async () => {
  const data = await fetchBootstrap();
  tasks.value = data.tasks;
});
</script>
