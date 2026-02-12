<template>
  <aside class="side-nav">
    <div class="brand">
      <div class="brand-mark">CR</div>
      <div class="brand-text">
        <div class="brand-title">Cloudreve Sync</div>
        <div class="brand-subtitle">{{ t("nav.subtitle") }}</div>
      </div>
    </div>
    <nav class="nav-list">
      <RouterLink v-for="item in items" :key="item.path" :to="item.path" class="nav-item">
        <el-icon><component :is="item.icon" /></el-icon>
        <span>{{ item.label }}</span>
      </RouterLink>
    </nav>
    <div class="nav-footer">
      <div class="hint">{{ t("nav.hint") }}</div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { RouterLink } from "vue-router";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  DataLine,
  FolderOpened,
  WarningFilled,
  Document,
  Setting,
  InfoFilled
} from "@element-plus/icons-vue";

const { t } = useI18n();
const items = computed(() => [
  { label: t("nav.dashboard"), path: "/", icon: DataLine },
  { label: t("nav.tasks"), path: "/tasks", icon: FolderOpened },
  { label: t("nav.conflicts"), path: "/conflicts", icon: WarningFilled },
  { label: t("nav.logs"), path: "/logs", icon: Document },
  { label: t("nav.settings"), path: "/settings", icon: Setting },
  { label: t("nav.about"), path: "/about", icon: InfoFilled }
]);
</script>
