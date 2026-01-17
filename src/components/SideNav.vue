<template>
  <aside class="side-nav">
    <div class="brand">
      <div class="brand-mark">CR</div>
      <div class="brand-text">
        <div class="brand-title">Cloudreve Sync</div>
        <div class="brand-subtitle">双向同步控制台</div>
      </div>
    </div>
    <nav class="nav-list">
      <RouterLink v-for="item in items" :key="item.path" :to="item.path" class="nav-item">
        <el-icon><component :is="item.icon" /></el-icon>
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>
    <div class="nav-footer">
      <div class="hint">不会覆盖文件 · 冲突双保留</div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { RouterLink } from "vue-router";
import {
  DataLine,
  FolderOpened,
  WarningFilled,
  Document,
  Setting,
  InfoFilled
} from "@element-plus/icons-vue";

const items = [
  { label: "概览", path: "/", icon: DataLine },
  { label: "同步任务", path: "/tasks", icon: FolderOpened },
  { label: "冲突中心", path: "/conflicts", icon: WarningFilled },
  { label: "活动日志", path: "/logs", icon: Document },
  { label: "设置", path: "/settings", icon: Setting },
  { label: "关于", path: "/about", icon: InfoFilled }
];
</script>
