<template>
  <section class="conflicts-view">
    <div class="conflict-grid">
      <el-card class="panel">
        <div class="panel-header">
          <div>
            <div class="panel-title">冲突列表</div>
            <div class="panel-subtitle">双保留策略下的冲突记录</div>
          </div>
          <el-button @click="refresh">刷新</el-button>
        </div>
        <el-input v-model="search" placeholder="筛选文件名 / 目录" />
        <el-table :data="filtered" height="420" class="table-flat" @row-click="selectConflict">
          <el-table-column prop="name" label="文件名" />
          <el-table-column prop="task" label="任务" width="120" />
          <el-table-column prop="time" label="时间" width="160" />
          <el-table-column prop="status" label="状态" width="100" />
        </el-table>
      </el-card>

      <el-card class="panel detail-panel" v-if="selected">
        <div class="panel-header">
          <div>
            <div class="panel-title">冲突详情</div>
            <div class="panel-subtitle">原文件与冲突副本双保留</div>
          </div>
          <el-button type="primary">标记已处理</el-button>
        </div>
        <div class="conflict-summary">
          <div>
            <div class="summary-label">原文件名（云端）</div>
            <div class="summary-value">{{ selected.name }}</div>
          </div>
          <div>
            <div class="summary-label">冲突副本</div>
            <div class="summary-value">{{ selected.name }} (conflict-{{ selected.device || "LOCAL" }})</div>
          </div>
          <div>
            <div class="summary-label">目录路径</div>
            <div class="summary-value">{{ selected.path }}</div>
          </div>
        </div>
        <div class="compare-grid">
          <el-card class="compare-card">
            <div class="compare-title">云端版本</div>
            <div class="compare-item">mtime/sha256 见日志与元数据</div>
          </el-card>
          <el-card class="compare-card">
            <div class="compare-title">冲突副本</div>
            <div class="compare-item">mtime/sha256 见日志与元数据</div>
          </el-card>
        </div>
        <div class="conflict-actions">
          <el-button>下载云端版本</el-button>
          <el-button>打开文件目录</el-button>
          <el-button type="primary" plain>复制 sha256</el-button>
        </div>
      </el-card>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { ConflictItem } from "../services/types";
import { listConflicts } from "../services/api";

const conflicts = ref<ConflictItem[]>([]);
const selected = ref<ConflictItem | null>(null);
const search = ref("");

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
</script>
