<template>
  <div class="app-shell">
    <SideNav />
    <div class="main-stack">
      <TopBar :title="pageMeta.title" :subtitle="pageMeta.subtitle" />
      <main class="main-content">
        <RouterView v-slot="{ Component }">
          <transition name="fade-rise" mode="out-in">
            <component :is="Component" />
          </transition>
        </RouterView>
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import SideNav from "./components/SideNav.vue";
import TopBar from "./components/TopBar.vue";

const route = useRoute();

const pageMeta = computed(() => {
  return {
    title: (route.meta.title as string) || "概览",
    subtitle: (route.meta.subtitle as string) || "同步状态与活动总览"
  };
});
</script>
