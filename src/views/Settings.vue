<template>
  <section class="settings-view">
    <div class="settings-grid">
      <el-card class="panel">
        <div class="panel-title">{{ t("settings.general") }}</div>
        <el-switch v-model="autostart" :active-text="t('settings.autostart')" />
        <el-switch v-model="tray" :active-text="t('settings.tray')" />
        <el-select v-model="language" :placeholder="t('settings.language')">
          <el-option :label="t('settings.languageZh')" value="zh" />
          <el-option :label="t('settings.languageEn')" value="en" />
        </el-select>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">{{ t("settings.network") }}</div>
        <el-input v-model="proxy" :placeholder="t('settings.proxyPlaceholder')" />
        <el-input-number v-model="retries" :min="0" :placeholder="t('settings.retriesPlaceholder')" />
        <el-select v-model="backoff" :placeholder="t('settings.backoffPlaceholder')">
          <el-option :label="t('settings.backoffExponential')" value="指数退避" />
          <el-option :label="t('settings.backoffLinear')" value="线性退避" />
          <el-option :label="t('settings.backoffFixed')" value="固定间隔" />
        </el-select>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">{{ t("settings.performance") }}</div>
        <div class="field-row">
          <span class="field-label">{{ t("settings.uploadConcurrency") }}</span>
          <el-input-number v-model="upload" :min="1" />
        </div>
        <div class="field-row">
          <span class="field-label">{{ t("settings.downloadConcurrency") }}</span>
          <el-input-number v-model="download" :min="1" />
        </div>
        <div class="field-row">
          <span class="field-label">{{ t("settings.shaThreads") }}</span>
          <el-input-number v-model="shaThreads" :min="1" />
        </div>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">{{ t("settings.security") }}</div>
        <el-button type="danger" plain @click="clearAllCredentials">{{ t("settings.clearCredentials") }}</el-button>
        <el-switch v-model="lockPause" :active-text="t('settings.lockPause')" />
      </el-card>
      <el-card class="panel">
        <div class="panel-title">{{ t("settings.advanced") }}</div>
        <el-switch v-model="debug" :active-text="t('settings.debug')" />
        <el-switch v-model="trace" :active-text="t('settings.trace')" />
      </el-card>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { useI18n } from "vue-i18n";
import { clearCredentials, getSettings, saveSettings } from "../services/api";
import { applyLocale } from "../i18n";

const { t } = useI18n();

const autostart = ref(true);
const tray = ref(true);
const language = ref("zh");
const proxy = ref("");
const retries = ref(5);
const backoff = ref("指数退避");
const upload = ref(4);
const download = ref(4);
const shaThreads = ref(4);
const lockPause = ref(false);
const debug = ref(false);
const trace = ref(false);

const buildPayload = () => ({
  autostart: autostart.value,
  tray: tray.value,
  language: language.value,
  proxy: proxy.value,
  retries: retries.value,
  backoff: backoff.value,
  upload: upload.value,
  download: download.value,
  sha_threads: shaThreads.value,
  lock_pause: lockPause.value,
  debug: debug.value,
  trace: trace.value
});

let loaded = false;
let saveTimer: number | null = null;
const scheduleSave = () => {
  if (!loaded) return;
  if (saveTimer) {
    window.clearTimeout(saveTimer);
  }
  saveTimer = window.setTimeout(async () => {
    try {
      await saveSettings(buildPayload());
      ElMessage.success(t("settings.saved"));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      ElMessage.error(t("common.saveFailed", { msg: message }));
    }
  }, 500);
};

const clearAllCredentials = async () => {
  await clearCredentials();
  ElMessage.success(t("settings.cleared"));
};

onMounted(async () => {
  const settings = await getSettings();
  autostart.value = settings.autostart;
  tray.value = settings.tray;
  language.value = settings.language;
  proxy.value = settings.proxy;
  retries.value = settings.retries;
  backoff.value = settings.backoff;
  upload.value = settings.upload;
  download.value = settings.download;
  shaThreads.value = settings.sha_threads;
  lockPause.value = settings.lock_pause;
  debug.value = settings.debug;
  trace.value = settings.trace;
  applyLocale(settings.language);
  loaded = true;
});

watch(language, value => {
  applyLocale(value);
});

watch(
  [
    autostart,
    tray,
    language,
    proxy,
    retries,
    backoff,
    upload,
    download,
    shaThreads,
    lockPause,
    debug,
    trace
  ],
  () => {
    scheduleSave();
  }
);
</script>
