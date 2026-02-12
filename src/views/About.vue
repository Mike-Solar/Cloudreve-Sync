<template>
  <section class="about-view">
    <el-card class="panel">
      <div class="panel-title">Cloudreve Sync</div>
      <p class="about-text">
        {{ t("about.desc") }}
      </p>
      <div class="about-grid">
        <div>
          <div class="summary-label">{{ t("about.version") }}</div>
          <div class="summary-value">{{ diagnostics?.app_version || "—" }}</div>
        </div>
        <div>
          <div class="summary-label">{{ t("about.protocol") }}</div>
          <div class="summary-value">Cloudreve REST API</div>
        </div>
        <div>
          <div class="summary-label">{{ t("about.strategy") }}</div>
          <div class="summary-value">{{ t("about.strategyValue") }}</div>
        </div>
      </div>
      <div class="about-actions">
        <el-button @click="showDiagnostics = true">{{ t("about.showDiagnostics") }}</el-button>
        <el-button type="primary" @click="openDocs">{{ t("about.openDocs") }}</el-button>
      </div>
    </el-card>

    <el-dialog v-model="showDiagnostics" :title="t('about.diagnosticsTitle')" width="520px">
      <el-descriptions :column="1" border v-if="diagnostics">
        <el-descriptions-item :label="t('about.version')">{{ diagnostics.app_version }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.os')">{{ diagnostics.os }} / {{ diagnostics.arch }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.configDir')">{{ diagnostics.config_dir }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.db')">{{ diagnostics.db_path }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.accounts')">{{ diagnostics.accounts }}</el-descriptions-item>
        <el-descriptions-item :label="t('about.tasks')">{{ diagnostics.tasks }}</el-descriptions-item>
      </el-descriptions>
      <template #footer>
        <el-button @click="showDiagnostics = false">{{ t("about.close") }}</el-button>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getDiagnostics, openExternal } from "../services/api";
import type { DiagnosticInfo } from "../services/types";

const diagnostics = ref<DiagnosticInfo | null>(null);
const showDiagnostics = ref(false);
const { t } = useI18n();

const openDocs = async () => {
  await openExternal("https://docs.cloudreve.org/");
};

onMounted(async () => {
  diagnostics.value = await getDiagnostics();
});
</script>
