<template>
  <header class="top-bar">
    <div class="top-left">
      <div class="page-title">{{ title }}</div>
      <div class="page-subtitle">{{ subtitle }}</div>
    </div>
    <div class="top-actions">
      <el-input v-model="query" placeholder="搜索任务 / 文件 / 日志" class="search-input" size="large" />
      <div class="chip">
        <span class="chip-label">网络</span>
        <strong>稳定</strong>
      </div>
      <el-dropdown>
        <span class="account">
          账户：mikesolar
          <el-icon><arrow-down /></el-icon>
        </span>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item>切换站点</el-dropdown-item>
            <el-dropdown-item>退出登录</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ArrowDown } from "@element-plus/icons-vue";

defineProps<{ title: string; subtitle: string }>();

const query = ref("");
</script>
