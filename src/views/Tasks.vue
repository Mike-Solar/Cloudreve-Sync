<template>
  <section class="tasks-view">
    <div class="toolbar">
      <el-button type="primary" @click="wizardVisible = true">+ {{ t("tasks.newTask") }}</el-button>
      <div class="toolbar-actions">
        <el-button @click="refresh">{{ t("tasks.refresh") }}</el-button>
      </div>
      <div class="toolbar-filters">
        <el-checkbox v-model="onlyErrors">{{ t("tasks.onlyErrors") }}</el-checkbox>
        <el-checkbox v-model="onlyConflicts">{{ t("tasks.onlyConflicts") }}</el-checkbox>
        <el-checkbox v-model="recent">{{ t("tasks.recentActive") }}</el-checkbox>
      </div>
    </div>

    <el-table :data="filtered" class="table-flat">
      <el-table-column prop="name" :label="t('tasks.tableName')" width="160" />
      <el-table-column prop="mode" :label="t('tasks.tableMode')" width="100">
        <template #default="{ row }">{{ localizedMode(row.mode) }}</template>
      </el-table-column>
      <el-table-column prop="local_path" :label="t('tasks.tableLocal')" />
      <el-table-column prop="remote_path" :label="t('tasks.tableRemote')" />
      <el-table-column prop="progress_text" :label="t('tasks.tableProgress')" width="240" />
      <el-table-column :label="t('tasks.tableStatus')" width="140">
        <template #default="{ row }">
          <el-tag :type="statusTone(row.status)" effect="dark">{{ localizedStatus(row.status) }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="t('tasks.tableActions')" width="220">
        <template #default="{ row }">
          <el-button size="small" @click="toggleSync(row)">
            {{ isRunningStatus(row.status) ? t("dashboard.pause") : t("dashboard.sync") }}
          </el-button>
          <el-button size="small" plain @click="removeTask(row)">{{ t("tasks.remove") }}</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="wizardVisible" :title="t('tasks.wizardTitle')" width="720px">
      <el-steps :active="step" finish-status="success" align-center>
        <el-step :title="t('tasks.stepAccount')" />
        <el-step :title="t('tasks.stepDirectory')" />
        <el-step :title="t('tasks.stepStrategy')" />
        <el-step :title="t('tasks.stepFirstSync')" />
      </el-steps>

      <div class="wizard-body" v-if="step === 0">
        <el-select
          v-if="accounts.length"
          v-model="selectedAccountKey"
          :placeholder="t('tasks.selectExisting')"
          clearable
          @change="applyAccountSelection"
        >
          <el-option :key="NEW_ACCOUNT_KEY" :label="t('tasks.connectNew')" :value="NEW_ACCOUNT_KEY" />
          <el-option
            v-for="item in accounts"
            :key="item.account_key"
            :label="`${item.email} · ${item.base_url}`"
            :value="item.account_key"
          />
        </el-select>
        <el-input v-model="wizard.base_url" placeholder="Cloudreve Base URL" :disabled="usingExistingAccount" />
        <el-input v-model="wizard.email" :placeholder="t('tasks.emailPlaceholder')" :disabled="usingExistingAccount" />
        <el-input
          v-if="!usingExistingAccount"
          v-model="wizard.password"
          :placeholder="t('tasks.passwordPlaceholder')"
          type="password"
          show-password
        />
        <el-input v-if="!usingExistingAccount" v-model="wizard.captcha" :placeholder="t('tasks.captchaPlaceholder')" />
        <el-input
          v-if="!usingExistingAccount"
          v-model="wizard.ticket"
          :placeholder="t('tasks.ticketPlaceholder')"
        />
        <el-button
          type="primary"
          :loading="loginLoading"
          @click="usingExistingAccount ? doUseAccount() : doLogin()"
        >
          {{ usingExistingAccount ? t("tasks.useSavedAccount") : t("tasks.loginAndTest") }}
        </el-button>
        <el-button
          v-if="!usingExistingAccount"
          :loading="captchaLoading"
          :disabled="captchaCooldown > 0"
          plain
          @click="fetchCaptcha"
        >
          {{
            captchaCooldown > 0
              ? t("tasks.refreshCaptchaCountdown", { seconds: captchaCooldown })
              : t("tasks.refreshCaptcha")
          }}
        </el-button>
        <el-alert
          v-if="!usingExistingAccount"
          type="info"
          show-icon
          :closable="false"
          :title="t('tasks.captchaHint')"
        />
        <el-alert v-if="loginError" type="error" show-icon :closable="false" :title="loginError" />
        <div v-if="captchaImage && !usingExistingAccount" class="captcha-preview">
          <img :src="captchaImage" alt="captcha" />
        </div>
      </div>

      <div class="wizard-body" v-else-if="step === 1">
        <el-input v-model="wizard.task_name" :placeholder="t('tasks.taskNamePlaceholder')" />
        <el-input v-model="wizard.local_root" :placeholder="t('tasks.localDirPlaceholder')">
          <template #append>
            <el-button @click="browseLocalRoot">{{ t("tasks.browse") }}</el-button>
          </template>
        </el-input>
        <el-input v-model="wizard.remote_root_uri" :placeholder="t('tasks.remoteDirPlaceholder')">
          <template #append>
            <el-button :disabled="!wizard.account_key" @click="openRemoteBrowser">{{ t("tasks.browse") }}</el-button>
          </template>
        </el-input>
      </div>

      <div class="wizard-body" v-else-if="step === 2">
        <el-radio-group v-model="wizard.mode">
          <el-radio label="Bidirectional">{{ t("tasks.modeBoth") }}</el-radio>
          <el-radio label="UploadOnly">{{ t("tasks.modeUploadOnly") }}</el-radio>
          <el-radio label="DownloadOnly">{{ t("tasks.modeDownloadOnly") }}</el-radio>
        </el-radio-group>
        <el-alert type="info" show-icon :title="t('tasks.strategyHint')" />
      </div>

      <div class="wizard-body" v-else>
        <el-radio-group v-model="wizard.first_sync">
          <el-radio label="sync">{{ t("tasks.firstSyncNow") }}</el-radio>
          <el-radio label="index">{{ t("tasks.firstSyncIndexOnly") }}</el-radio>
        </el-radio-group>
        <el-input-number v-model="wizard.sync_interval_secs" :min="5" :label="t('tasks.syncIntervalLabel')" />
      </div>

      <template #footer>
        <div class="wizard-footer">
          <el-button @click="wizardVisible = false">{{ t("tasks.cancel") }}</el-button>
          <el-button :disabled="step === 0" @click="step--">{{ t("tasks.previous") }}</el-button>
          <el-button v-if="step < 3" type="primary" :loading="nextLoading" @click="goNext">
            {{ t("tasks.next") }}
          </el-button>
          <el-button v-else type="primary" :loading="createLoading" @click="submitTask">
            {{ t("tasks.createTaskAction") }}
          </el-button>
        </div>
      </template>
    </el-dialog>

    <el-dialog v-model="twoFaVisible" :title="t('tasks.twoFaTitle')" width="420px">
      <div class="wizard-body">
        <el-input v-model="twoFaCode" :placeholder="t('tasks.twoFaPlaceholder')" />
        <el-alert
          type="info"
          show-icon
          :closable="false"
          :title="t('tasks.twoFaHint')"
        />
      </div>
      <template #footer>
        <div class="wizard-footer">
          <el-button @click="twoFaVisible = false">{{ t("tasks.cancel") }}</el-button>
          <el-button type="primary" :loading="twoFaLoading" @click="submitTwoFa">
            {{ t("tasks.verifyAndLogin") }}
          </el-button>
        </div>
      </template>
    </el-dialog>

    <el-dialog v-model="remoteBrowserVisible" :title="t('tasks.remotePickerTitle')" width="640px">
      <div class="wizard-body">
        <div class="remote-browser-header">
          <el-button size="small" plain @click="goRemoteParent">{{ t("tasks.parent") }}</el-button>
          <span class="remote-browser-path">{{ remoteBrowserUri }}</span>
        </div>
        <el-table :data="remoteBrowserEntries" height="320" v-loading="remoteBrowserLoading">
          <el-table-column :label="t('tasks.name')">
            <template #default="{ row }">
              <span>{{ row.name }}</span>
            </template>
          </el-table-column>
          <el-table-column :label="t('tasks.type')" width="120">
            <template #default="{ row }">
              {{ row.is_dir ? t("tasks.dir") : t("tasks.file") }}
            </template>
          </el-table-column>
          <el-table-column :label="t('tasks.tableActions')" width="140">
            <template #default="{ row }">
              <el-button size="small" :disabled="!row.is_dir" @click="enterRemote(row)">
                {{ t("tasks.open") }}
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
      <template #footer>
        <div class="wizard-footer">
          <el-button @click="remoteBrowserVisible = false">{{ t("tasks.cancel") }}</el-button>
          <el-button type="primary" @click="selectRemoteCurrent">{{ t("tasks.selectCurrentDir") }}</el-button>
        </div>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useI18n } from "vue-i18n";
import type { TaskItem, AccountItem, RemoteEntry, TaskRuntimePayload } from "../services/types";
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
const recent = ref(false);
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
let unlistenTaskRuntime: UnlistenFn | null = null;
const { t } = useI18n();

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
  mode: "Bidirectional",
  first_sync: "sync",
  sync_interval_secs: 60
});

const refresh = async () => {
  tasks.value = await listTasks();
};

const applyTaskRuntime = (payload: TaskRuntimePayload) => {
  const index = tasks.value.findIndex(item => item.id === payload.task_id);
  if (index < 0) return;
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

const localizedMode = (mode: string) => {
  if (mode === "双向" || mode === "Bidirectional") return t("tasks.modeBoth");
  if (mode === "单向→" || mode === "UploadOnly") return t("tasks.modeUploadOnly");
  if (mode === "单向←" || mode === "DownloadOnly") return t("tasks.modeDownloadOnly");
  return mode;
};

const statusTone = (status: string) => {
  if (isRunningStatus(status)) return "success";
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
  return t("tasks.unknownError");
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
      ElMessage.warning(t("tasks.twoFaRequired"));
      return;
    }
    await testConnection(result.account_key, wizard.value.base_url);
    wizard.value.account_key = result.account_key;
    await loadAccounts();
    ElMessage.success(t("tasks.loginSuccess"));
  } catch (err) {
    const message = t("tasks.loginFailed", { msg: formatError(err) });
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    loginLoading.value = false;
  }
};

const doUseAccount = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error(t("tasks.selectExistingFirst"));
    return;
  }
  try {
    loginLoading.value = true;
    loginError.value = "";
    await testConnection(wizard.value.account_key, wizard.value.base_url);
    ElMessage.success(t("tasks.connectSuccess"));
  } catch (err) {
    const message = t("tasks.connectFailed", { msg: formatError(err) });
    ElMessage.error(message);
    loginError.value = message;
  } finally {
    loginLoading.value = false;
  }
};

const fetchCaptcha = async () => {
  if (!wizard.value.base_url) {
    ElMessage.warning(t("tasks.fillBaseUrlFirst"));
    return;
  }
  try {
    captchaLoading.value = true;
    loginError.value = "";
    const data: any = await getCaptcha(wizard.value.base_url);
    wizard.value.ticket = data.ticket;
    wizard.value.captcha = "";
    captchaImage.value = data.image;
    ElMessage.success(t("tasks.captchaRefreshed"));
    captchaCooldown.value = 15;
  } catch (err) {
    const message = t("tasks.captchaRefreshFailed", { msg: formatError(err) });
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
      title: t("tasks.pickLocalDir")
    })) as string | string[] | null;
    if (typeof selected === "string") {
      wizard.value.local_root = selected;
    } else if (Array.isArray(selected) && selected.length > 0) {
      wizard.value.local_root = selected[0];
    }
  } catch (err) {
    ElMessage.error(t("tasks.openLocalDirFailed", { msg: formatError(err) }));
  }
};

const normalizeRemoteUri = (value: string) => {
  const decoded = (() => {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  })();
  if (!value) return "cloudreve://my";
  if (decoded.startsWith("cloudreve://")) return decoded;
  if (decoded.startsWith("/")) return `cloudreve://my${decoded}`;
  return `cloudreve://my/${decoded}`;
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
    ElMessage.error(t("tasks.loginRequiredForRemote"));
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
    ElMessage.error(t("tasks.listRemoteFailed", { msg: formatError(err) }));
  } finally {
    remoteBrowserLoading.value = false;
  }
};

const openRemoteBrowser = async () => {
  if (!wizard.value.account_key) {
    ElMessage.error(t("tasks.loginRequiredForRemote"));
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
    ElMessage.error(t("tasks.enterTwoFa"));
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
      ElMessage.error(t("tasks.twoFaFailedRetry"));
      return;
    }
    await testConnection(result.account_key, wizard.value.base_url);
    wizard.value.account_key = result.account_key;
    await loadAccounts();
    twoFaVisible.value = false;
    twoFaSessionId.value = "";
    twoFaCode.value = "";
    ElMessage.success(t("tasks.loginSuccess"));
  } catch (err) {
    const message = t("tasks.twoFaFailed", { msg: formatError(err) });
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
    ElMessage.error(t("tasks.fillBaseUrl"));
    return false;
  }
  if (!wizard.value.email.trim()) {
    ElMessage.error(t("tasks.fillEmail"));
    return false;
  }
  if (!usingExistingAccount.value && !wizard.value.password) {
    ElMessage.error(t("tasks.fillPassword"));
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
    ElMessage.error(t("tasks.loginRequiredForRemote"));
    return;
  }
  try {
    createLoading.value = true;
    const createdTaskId = await createTask({
      name: wizard.value.task_name || t("tasks.defaultTaskName"),
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
      await runSync({ task_id: createdTaskId });
      await refresh();
    }
    ElMessage.success(t("tasks.taskCreated"));
  } catch (err) {
    ElMessage.error(t("tasks.createTaskFailed", { msg: formatError(err) }));
  } finally {
    createLoading.value = false;
  }
};

const toggleSync = async (row: TaskItem) => {
  if (isRunningStatus(row.status)) {
    await stopSync({ task_id: row.id });
  } else {
    await runSync({ task_id: row.id });
  }
  await refresh();
};

const removeTask = async (row: TaskItem) => {
  try {
    await ElMessageBox.confirm(
      t("tasks.removeTaskConfirm", { name: row.name }),
      t("tasks.removeTaskTitle"),
      {
        type: "warning",
        confirmButtonText: t("tasks.removeConfirm"),
        cancelButtonText: t("tasks.cancel")
      }
    );
  } catch {
    return;
  }
  try {
    await deleteTask({ task_id: row.id });
    await refresh();
    ElMessage.success(t("tasks.taskRemoved"));
  } catch (err) {
    ElMessage.error(t("tasks.removeFailed", { msg: formatError(err) }));
  }
};

onMounted(async () => {
  const data = await fetchBootstrap();
  tasks.value = data.tasks;
  await loadAccounts();
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
