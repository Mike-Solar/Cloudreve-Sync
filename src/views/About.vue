<template>
  <section class="about-view">
    <el-card class="panel">
      <div class="panel-title">Cloudreve Sync</div>
      <p class="about-text">
        本客户端不覆盖文件，不自动解决冲突。冲突发生时始终保留双方文件副本，
        所有原始时间戳通过 metadata 保存与恢复。
      </p>
      <div class="about-grid">
        <div>
          <div class="summary-label">版本</div>
          <div class="summary-value">{{ diagnostics?.app_version || "—" }}</div>
        </div>
        <div>
          <div class="summary-label">后端协议</div>
          <div class="summary-value">Cloudreve REST API</div>
        </div>
        <div>
          <div class="summary-label">同步策略</div>
          <div class="summary-value">双向 · 冲突双保留 · 软删除</div>
        </div>
      </div>
      <div class="about-actions">
        <el-button @click="showDiagnostics = true">查看诊断信息</el-button>
        <el-button type="primary" @click="openDocs">打开文档</el-button>
      </div>
    </el-card>

    <el-dialog v-model="showDiagnostics" title="诊断信息" width="520px">
      <el-descriptions :column="1" border v-if="diagnostics">
        <el-descriptions-item label="版本">{{ diagnostics.app_version }}</el-descriptions-item>
        <el-descriptions-item label="系统">{{ diagnostics.os }} / {{ diagnostics.arch }}</el-descriptions-item>
        <el-descriptions-item label="配置目录">{{ diagnostics.config_dir }}</el-descriptions-item>
        <el-descriptions-item label="数据库">{{ diagnostics.db_path }}</el-descriptions-item>
        <el-descriptions-item label="账号数量">{{ diagnostics.accounts }}</el-descriptions-item>
        <el-descriptions-item label="任务数量">{{ diagnostics.tasks }}</el-descriptions-item>
      </el-descriptions>
      <template #footer>
        <el-button @click="showDiagnostics = false">关闭</el-button>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getDiagnostics, openExternal } from "../services/api";
import type { DiagnosticInfo } from "../services/types";

const diagnostics = ref<DiagnosticInfo | null>(null);
const showDiagnostics = ref(false);

const openDocs = async () => {
  await openExternal("https://docs.cloudreve.org/");
};

onMounted(async () => {
  diagnostics.value = await getDiagnostics();
});
</script>
