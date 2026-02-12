<template>
  <section class="conflicts-view">
    <div class="conflict-grid">
      <el-card class="panel">
        <div class="panel-header">
          <div>
            <div class="panel-title">{{ t("conflicts.listTitle") }}</div>
            <div class="panel-subtitle">{{ t("conflicts.listSub") }}</div>
          </div>
          <el-button @click="refresh">{{ t("conflicts.refresh") }}</el-button>
        </div>
        <el-input v-model="search" :placeholder="t('conflicts.filterPlaceholder')" />
        <el-table :data="filtered" height="420" class="table-flat" @row-click="selectConflict">
          <el-table-column prop="name" :label="t('conflicts.colName')" />
          <el-table-column prop="task" :label="t('conflicts.colTask')" width="120" />
          <el-table-column prop="time" :label="t('conflicts.colTime')" width="160" />
          <el-table-column prop="status" :label="t('conflicts.colStatus')" width="100" />
        </el-table>
      </el-card>

      <el-card class="panel detail-panel" v-if="selected">
        <div class="panel-header">
          <div>
            <div class="panel-title">{{ t("conflicts.detailTitle") }}</div>
            <div class="panel-subtitle">{{ t("conflicts.detailSub") }}</div>
          </div>
          <el-button type="primary" @click="markResolved">{{ t("conflicts.markResolved") }}</el-button>
        </div>
        <div class="conflict-summary">
          <div>
            <div class="summary-label">{{ t("conflicts.originalName") }}</div>
            <div class="summary-value">{{ selected.name }}</div>
          </div>
          <div>
            <div class="summary-label">{{ t("conflicts.conflictCopy") }}</div>
            <div class="summary-value">
              {{ selected.name }} (conflict-{{ selected.device || t("conflicts.localDevice") }})
            </div>
          </div>
          <div>
            <div class="summary-label">{{ t("conflicts.dirPath") }}</div>
            <div class="summary-value">{{ selected.path }}</div>
          </div>
        </div>
        <div class="compare-grid">
          <el-card class="compare-card">
            <div class="compare-title">{{ t("conflicts.remoteVersion") }}</div>
            <div class="compare-item">{{ t("conflicts.versionHint") }}</div>
          </el-card>
          <el-card class="compare-card">
            <div class="compare-title">{{ t("conflicts.localConflictVersion") }}</div>
            <div class="compare-item">{{ t("conflicts.versionHint") }}</div>
          </el-card>
        </div>
        <div class="conflict-actions">
          <el-button @click="downloadRemote">{{ t("conflicts.downloadRemote") }}</el-button>
          <el-button @click="openFolder">{{ t("conflicts.openFolder") }}</el-button>
          <el-button type="primary" plain @click="copySha256">{{ t("conflicts.copySha256") }}</el-button>
        </div>
      </el-card>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { useI18n } from "vue-i18n";
import type { ConflictItem } from "../services/types";
import { downloadConflictRemote, hashLocalFile, listConflicts, markConflictResolved, openLocalPath } from "../services/api";

const conflicts = ref<ConflictItem[]>([]);
const selected = ref<ConflictItem | null>(null);
const search = ref("");
const { t } = useI18n();

const refresh = async () => {
  conflicts.value = await listConflicts();
  selected.value = conflicts.value[0] ?? null;
};

onMounted(refresh);

const filtered = computed(() => {
  const term = search.value.trim().toLowerCase();
  if (!term) return conflicts.value;
  return conflicts.value.filter(item =>
    `${item.name} ${item.path}`.toLowerCase().includes(term)
  );
});

const selectConflict = (row: ConflictItem) => {
  selected.value = row;
};

const markResolved = async () => {
  if (!selected.value) return;
  await markConflictResolved(selected.value.task_id, selected.value.conflict_relpath);
  await refresh();
  ElMessage.success(t("conflicts.marked"));
};

const downloadRemote = async () => {
  if (!selected.value) return;
  await downloadConflictRemote(selected.value.task_id, selected.value.original_relpath);
  ElMessage.success(t("conflicts.openedDownload"));
};

const openFolder = async () => {
  if (!selected.value) return;
  await openLocalPath(selected.value.local_dir);
};

const copySha256 = async () => {
  if (!selected.value) return;
  const hash = await hashLocalFile(selected.value.local_path);
  await navigator.clipboard.writeText(hash);
  ElMessage.success(t("conflicts.copied"));
};
</script>
