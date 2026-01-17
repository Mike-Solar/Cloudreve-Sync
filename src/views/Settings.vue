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
        <el-input v-model="retries" placeholder="最大重试次数" />
        <el-input v-model="backoff" placeholder="退避策略" />
      </el-card>
      <el-card class="panel">
        <div class="panel-title">性能</div>
        <el-input v-model="upload" placeholder="上传并发" />
        <el-input v-model="download" placeholder="下载并发" />
        <el-input v-model="shaThreads" placeholder="SHA256 线程数" />
      </el-card>
      <el-card class="panel">
        <div class="panel-title">安全</div>
        <el-button type="danger" plain>清除登录凭据</el-button>
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
import { ref } from "vue";

const autostart = ref(true);
const tray = ref(true);
const language = ref("zh");
const proxy = ref("");
const retries = ref("5");
const backoff = ref("指数退避");
const upload = ref("4");
const download = ref("4");
const shaThreads = ref("4");
const lockPause = ref(false);
const debug = ref(false);
const trace = ref(false);
</script>
