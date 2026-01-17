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
          <el-button size="small" plain @click="removeTask(row)">移除</el-button>
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
        <el-select
          v-if="accounts.length"
          v-model="selectedAccountKey"
          placeholder="选择已有账号（可选）"
          clearable
          @change="applyAccountSelection"
        >
          <el-option :key="NEW_ACCOUNT_KEY" label="连接新账号" :value="NEW_ACCOUNT_KEY" />
          <el-option
            v-for="item in accounts"
            :key="item.account_key"
            :label="`${item.email} · ${item.base_url}`"
            :value="item.account_key"
          />
        </el-select>
        <el-input v-model="wizard.base_url" placeholder="Cloudreve Base URL" :disabled="usingExistingAccount" />
        <el-input v-model="wizard.email" placeholder="邮箱" :disabled="usingExistingAccount" />
        <el-input
          v-if="!usingExistingAccount"
          v-model="wizard.password"
          placeholder="密码"
          type="password"
          show-password
        />
        <el-input v-if="!usingExistingAccount" v-model="wizard.captcha" placeholder="验证码" />
        <el-input v-if="!usingExistingAccount" v-model="wizard.ticket" placeholder="Captcha Ticket（自动填充）" />
        <el-button
          type="primary"
          :loading="loginLoading"
          @click="usingExistingAccount ? doUseAccount() : doLogin()"
        >
          {{ usingExistingAccount ? "使用已保存账号" : "登录并测试连接" }}
        </el-button>
        <el-button
          v-if="!usingExistingAccount"
          :loading="captchaLoading"
          :disabled="captchaCooldown > 0"
          plain
          @click="fetchCaptcha"
        >
          {{ captchaCooldown > 0 ? `刷新验证码 (${captchaCooldown}s)` : "刷新验证码" }}
        </el-button>
        <el-alert
          v-if="!usingExistingAccount"
          type="info"
          show-icon
          :closable="false"
          title="请先填写 Base URL，再点击“刷新验证码”获取图片。"
        />
        <el-alert v-if="loginError" type="error" show-icon :closable="false" :title="loginError" />
        <div v-if="captchaImage && !usingExistingAccount" class="captcha-preview">
          <img :src="captchaImage" alt="captcha" />
        </div>
      </div>

      <div class="wizard-body" v-else-if="step === 1">
        <el-input v-model="wizard.task_name" placeholder="任务名称" />
        <el-input v-model="wizard.local_root" placeholder="本地目录">
          <template #append>
            <el-button @click="browseLocalRoot">浏览</el-button>
          </template>
        </el-input>
        <el-input v-model="wizard.remote_root_uri" placeholder="云端目录 (URI 或路径)">
          <template #append>
            <el-button :disabled="!wizard.account_key" @click="openRemoteBrowser">浏览</el-button>
          </template>
        </el-input>
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
          <el-button v-if="step < 3" type="primary" :loading="nextLoading" @click="goNext">
            下一步
          </el-button>
          <el-button v-else type="primary" :loading="createLoading" @click="submitTask">
            创建任务
          </el-button>
        </div>
      </template>
    </el-dialog>

    <el-dialog v-model="twoFaVisible" title="需要两步验证" width="420px">
      <div class="wizard-body">
        <el-input v-model="twoFaCode" placeholder="请输入 2FA 验证码" />
        <el-alert
          type="info"
          show-icon
          :closable="false"
          title="请输入账号的两步验证码（TOTP）。"
        />
      </div>
      <template #footer>
        <div class="wizard-footer">
          <el-button @click="twoFaVisible = false">取消</el-button>
          <el-button type="primary" :loading="twoFaLoading" @click="submitTwoFa">
            验证并登录
          </el-button>
        </div>
      </template>
    </el-dialog>

    <el-dialog v-model="remoteBrowserVisible" title="选择云端目录" width="640px">
      <div class="wizard-body">
        <div class="remote-browser-header">
          <el-button size="small" plain @click="goRemoteParent">上一级</el-button>
          <span class="remote-browser-path">{{ remoteBrowserUri }}</span>
        </div>
        <el-table :data="remoteBrowserEntries" height="320" v-loading="remoteBrowserLoading">
          <el-table-column label="名称">
            <template #default="{ row }">
              <span>{{ row.name }}</span>
            </template>
          </el-table-column>
          <el-table-column label="类型" width="120">
            <template #default="{ row }">
              {{ row.is_dir ? "目录" : "文件" }}
            </template>
          </el-table-column>
          <el-table-column label="操作" width="140">
            <template #default="{ row }">
              <el-button size="small" :disabled="!row.is_dir" @click="enterRemote(row)">
                打开
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
      <template #footer>
        <div class="wizard-footer">
          <el-button @click="remoteBrowserVisible = false">取消</el-button>
          <el-button type="primary" @click="selectRemoteCurrent">选择当前目录</el-button>
        </div>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import type { TaskItem, AccountItem, RemoteEntry } from "../services/types";
import {
  createTask,
  deleteTask,
  fetchBootstrap,
  finishSignInWith2fa,
  listRemoteEntries,
  listAccounts,
  listTasks,
  login,
  runSync,
  stopSync,
  testConnection,
  getCaptcha
} from "../services/api";

const tasks = ref<TaskItem[]>([]);
const accounts = ref<AccountItem[]>([]);
const selectedAccountKey = ref("");
const onlyErrors = ref(false);
const onlyConflicts = ref(false);
const recent = ref(true);
const wizardVisible = ref(false);
const step = ref(0);
const captchaImage = ref<string | null>(null);
const loginLoading = ref(false);
const captchaLoading = ref(false);
const nextLoading = ref(false);
const captchaCooldown = ref(0);
const loginError = ref("");
const twoFaVisible = ref(false);
const twoFaCode = ref("");
const twoFaSessionId = ref("");
const twoFaLoading = ref(false);
const remoteBrowserVisible = ref(false);
const remoteBrowserEntries = ref<RemoteEntry[]>([]);
const remoteBrowserUri = ref("cloudreve://my");
const remoteBrowserLoading = ref(false);
const createLoading = ref(false);
let refreshTimer: number | null = null;

const NEW_ACCOUNT_KEY = "__new__";

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

const loadAccounts = async () => {
  accounts.value = await listAccounts();
};

const isNewAccountSelected = computed(() => selectedAccountKey.value === NEW_ACCOUNT_KEY);
const usingExistingAccount = computed(
  () => selectedAccountKey.value !== "" && !isNewAccountSelected.value
);

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

const formatError = (err: unknown) => {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err && typeof err.message === "string") {
    return err.message;
  }
  return "未知错误";
};

const doLogin = async () => {
  try {
    loginLoading.value = true;
    loginError.value = "";
    const result = await login({
      base_url: wizard.value.base_url,
      email: wizard.value.email,
      password: wizard.value.password,
      captcha: wizard.value.captcha || undefined,
      ticket: wizard.value.ticket || undefined
    });
    if (result.status === "two_fa_required") {
      twoFaSessionId.value = result.session_id;
      twoFaCode.value = "";
      twoFaVisible.value = true;
      ElMessage.warning("需要两步验证，请输入验证码");
      return;
    }
    await testConnection(result.account_key, wizard.value.base_url);
    wizard.value.account_key = result.account_key;
    await loadAccounts();
    ElMessage.success("登录并连接成功");
  } catch (err) {
    const message = `登录失败：${formatError(err)}`;
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    loginLoading.value = false;
  }
};

const doUseAccount = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error("请选择已有账号");
    return;
  }
  try {
    loginLoading.value = true;
    loginError.value = "";
    await testConnection(wizard.value.account_key, wizard.value.base_url);
    ElMessage.success("连接成功");
  } catch (err) {
    const message = `连接失败：${formatError(err)}`;
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    loginLoading.value = false;
  }
};

const fetchCaptcha = async () => {
  if (!wizard.value.base_url) {
    ElMessage.warning("请先填写 Base URL");
    return;
  }
  try {
    captchaLoading.value = true;
    loginError.value = "";
    const data: any = await getCaptcha(wizard.value.base_url);
    wizard.value.ticket = data.ticket;
    wizard.value.captcha = "";
    captchaImage.value = data.image;
    ElMessage.success("验证码已刷新");
    captchaCooldown.value = 15;
  } catch (err) {
    const message = `刷新验证码失败：${formatError(err)}`;
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    captchaLoading.value = false;
  }
};

const browseLocalRoot = async () => {
  try {
    const selected = (await open({
      directory: true,
      multiple: false,
      title: "选择本地目录"
    })) as string | string[] | null;
    if (typeof selected === "string") {
      wizard.value.local_root = selected;
    } else if (Array.isArray(selected) && selected.length > 0) {
      wizard.value.local_root = selected[0];
    }
  } catch (err) {
    ElMessage.error(`打开本地目录失败：${formatError(err)}`);
  }
};

const normalizeRemoteUri = (value: string) => {
  if (!value) return "cloudreve://my";
  if (value.startsWith("cloudreve://")) return value;
  if (value.startsWith("/")) return `cloudreve://my${value}`;
  return `cloudreve://my/${value}`;
};

const parentRemoteUri = (uri: string) => {
  const trimmed = uri.startsWith("cloudreve://") ? uri.slice("cloudreve://".length) : uri;
  const parts = trimmed.split("/").filter(Boolean);
  if (parts.length <= 1) {
    return `cloudreve://${parts[0] || "my"}`;
  }
  parts.pop();
  return `cloudreve://${parts.join("/")}`;
};

const loadRemoteEntries = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error("请先登录并验证连接");
    return;
  }
  remoteBrowserLoading.value = true;
  try {
    const entries = await listRemoteEntries({
      account_key: wizard.value.account_key,
      base_url: wizard.value.base_url,
      uri: remoteBrowserUri.value
    });
    remoteBrowserEntries.value = entries.sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
  } catch (err) {
    ElMessage.error(`获取云端目录失败：${formatError(err)}`);
  } finally {
    remoteBrowserLoading.value = false;
  }
};

const openRemoteBrowser = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error("请先登录并验证连接");
    return;
  }
  remoteBrowserUri.value = normalizeRemoteUri(wizard.value.remote_root_uri);
  remoteBrowserVisible.value = true;
  await loadRemoteEntries();
};

const enterRemote = async (entry: RemoteEntry) => {
  if (!entry.is_dir) return;
  remoteBrowserUri.value = entry.uri;
  await loadRemoteEntries();
};

const goRemoteParent = async () => {
  const parent = parentRemoteUri(remoteBrowserUri.value);
  if (parent === remoteBrowserUri.value) return;
  remoteBrowserUri.value = parent;
  await loadRemoteEntries();
};

const selectRemoteCurrent = () => {
  wizard.value.remote_root_uri = remoteBrowserUri.value;
  remoteBrowserVisible.value = false;
};

const submitTwoFa = async () => {
  if (!twoFaCode.value.trim()) {
    ElMessage.error("请输入 2FA 验证码");
    return;
  }
  try {
    twoFaLoading.value = true;
    loginError.value = "";
    const result = await finishSignInWith2fa({
      base_url: wizard.value.base_url,
      email: wizard.value.email,
      session_id: twoFaSessionId.value,
      opt: twoFaCode.value.trim()
    });
    if (result.status !== "success") {
      ElMessage.error("2FA 验证失败，请重试");
      return;
    }
    await testConnection(result.account_key, wizard.value.base_url);
    wizard.value.account_key = result.account_key;
    await loadAccounts();
    twoFaVisible.value = false;
    twoFaSessionId.value = "";
    twoFaCode.value = "";
    ElMessage.success("登录并连接成功");
  } catch (err) {
    const message = `2FA 验证失败：${formatError(err)}`;
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    twoFaLoading.value = false;
  }
};

const applyAccountSelection = () => {
  if (selectedAccountKey.value === NEW_ACCOUNT_KEY) {
    selectedAccountKey.value = "";
    wizard.value.account_key = "";
    wizard.value.base_url = "";
    wizard.value.email = "";
    wizard.value.password = "";
    wizard.value.captcha = "";
    wizard.value.ticket = "";
    captchaImage.value = null;
    return;
  }
  const account = accounts.value.find(item => item.account_key === selectedAccountKey.value);
  if (!account) {
    wizard.value.account_key = "";
    return;
  }
  wizard.value.account_key = account.account_key;
  wizard.value.base_url = account.base_url;
  wizard.value.email = account.email;
  wizard.value.password = "";
  wizard.value.captcha = "";
  wizard.value.ticket = "";
  captchaImage.value = null;
};

const validateStepZero = () => {
  if (!wizard.value.base_url.trim()) {
    ElMessage.error("请填写 Base URL");
    return false;
  }
  if (!wizard.value.email.trim()) {
    ElMessage.error("请填写邮箱");
    return false;
  }
  if (!usingExistingAccount.value && !wizard.value.password) {
    ElMessage.error("请填写密码");
    return false;
  }
  return true;
};

const goNext = async () => {
  if (step.value !== 0) {
    step.value += 1;
    return;
  }
  if (!validateStepZero()) return;
  nextLoading.value = true;
  if (!wizard.value.account_key) {
    if (usingExistingAccount.value) {
      await doUseAccount();
    } else {
      await doLogin();
    }
  }
  if (wizard.value.account_key) {
    step.value += 1;
  }
  nextLoading.value = false;
};

const submitTask = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error("请先登录并验证连接");
    return;
  }
  try {
    createLoading.value = true;
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
    onlyErrors.value = false;
    onlyConflicts.value = false;
    recent.value = false;
    await refresh();
    if (wizard.value.first_sync === "sync") {
      const created = tasks.value.find(item => item.name === (wizard.value.task_name || "新任务"));
      if (created) {
        await runSync({ task_id: created.id });
        await refresh();
      }
    }
    ElMessage.success("任务已创建");
  } catch (err) {
    ElMessage.error(`创建任务失败：${formatError(err)}`);
  } finally {
    createLoading.value = false;
  }
};

const toggleSync = async (row: TaskItem) => {
  if (row.status === "Syncing") {
    await stopSync({ task_id: row.id });
  } else {
    await runSync({ task_id: row.id });
  }
  await refresh();
};

const removeTask = async (row: TaskItem) => {
  try {
    await ElMessageBox.confirm(
      `确定要移除任务 “${row.name}” 吗？`,
      "移除任务",
      {
        type: "warning",
        confirmButtonText: "移除",
        cancelButtonText: "取消"
      }
    );
  } catch {
    return;
  }
  try {
    await deleteTask({ task_id: row.id });
    await refresh();
    ElMessage.success("任务已移除");
  } catch (err) {
    ElMessage.error(`移除失败：${formatError(err)}`);
  }
};

onMounted(async () => {
  const data = await fetchBootstrap();
  tasks.value = data.tasks;
  await loadAccounts();
  refreshTimer = window.setInterval(async () => {
    await refresh();
  }, 1000);
});

onBeforeUnmount(() => {
  if (refreshTimer) {
    window.clearInterval(refreshTimer);
  }
});

watch(wizardVisible, visible => {
  if (visible) {
    loadAccounts();
    loginError.value = "";
  }
});

const tickCooldown = () => {
  if (captchaCooldown.value > 0) {
    captchaCooldown.value -= 1;
  }
};

const cooldownTimer = window.setInterval(tickCooldown, 1000);

onBeforeUnmount(() => {
  window.clearInterval(cooldownTimer);
});

</script>
