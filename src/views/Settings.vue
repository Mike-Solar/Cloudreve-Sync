<template>
  <section class="settings-view">
    <div class="settings-grid">
      <el-card class="panel">
        <div class="panel-title">通用</div>
        <el-switch v-model="autostart" active-text="开机自启动" />
        <el-switch v-model="tray" active-text="托盘图标" />
        <el-select v-model="language" placeholder="语言">
          <el-option label="简体中文" value="zh" />
          <el-option label="English" value="en" />
        </el-select>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">网络</div>
        <el-input v-model="proxy" placeholder="代理地址" />
        <el-input-number v-model="retries" :min="0" placeholder="最大重试次数" />
        <el-select v-model="backoff" placeholder="退避策略">
          <el-option label="指数退避" value="指数退避" />
          <el-option label="线性退避" value="线性退避" />
          <el-option label="固定间隔" value="固定间隔" />
        </el-select>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">性能</div>
        <div class="field-row">
          <span class="field-label">上传并发</span>
          <el-input-number v-model="upload" :min="1" />
        </div>
        <div class="field-row">
          <span class="field-label">下载并发</span>
          <el-input-number v-model="download" :min="1" />
        </div>
        <div class="field-row">
          <span class="field-label">SHA256 线程数</span>
          <el-input-number v-model="shaThreads" :min="1" />
        </div>
      </el-card>
      <el-card class="panel">
        <div class="panel-title">安全</div>
        <el-button type="danger" plain @click="clearAllCredentials">清除登录凭据</el-button>
        <el-switch v-model="lockPause" active-text="锁屏后暂停同步" />
      </el-card>
      <el-card class="panel">
        <div class="panel-title">高级</div>
        <el-switch v-model="debug" active-text="调试模式" />
        <el-switch v-model="trace" active-text="API Trace" />
      </el-card>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { clearCredentials, getSettings, saveSettings } from "../services/api";

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
    await saveSettings(buildPayload());
    ElMessage.success("设置已保存");
  }, 500);
};

const clearAllCredentials = async () => {
  await clearCredentials();
  ElMessage.success("登录凭据已清除");
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
  loaded = true;
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
