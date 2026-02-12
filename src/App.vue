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

    <el-dialog
      v-model="shareDialogVisible"
      :title="t('share.title')"
      width="520px"
      @close="handleShareDialogClose"
    >
      <div class="share-form">
        <div class="share-path">
          <div class="share-label">{{ t("share.localPath") }}</div>
          <div class="share-value">{{ shareForm.localPath }}</div>
        </div>
        <el-form :model="shareForm" label-width="88px">
          <el-form-item :label="t('share.password')">
            <el-input
              v-model="shareForm.password"
              :placeholder="t('share.passwordPlaceholder')"
              maxlength="32"
              show-word-limit
            />
          </el-form-item>
          <el-form-item :label="t('share.expire')">
            <el-select v-model="shareForm.expire" :placeholder="t('share.selectPlaceholder')">
              <el-option
                v-for="option in expireOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
        </el-form>
        <div v-if="shareForm.shareLink" class="share-result">
          <div class="share-label">{{ t("share.link") }}</div>
          <div class="share-link-row">
            <el-input v-model="shareForm.shareLink" readonly />
            <el-button @click="copyShareLink">{{ t("share.copy") }}</el-button>
            <el-button type="primary" @click="openShareLink">{{ t("share.open") }}</el-button>
          </div>
        </div>
        <div v-if="shareForm.error" class="share-error">{{ shareForm.error }}</div>
      </div>
      <template #footer>
        <el-button @click="shareDialogVisible = false">{{ t("share.close") }}</el-button>
        <el-button type="primary" :loading="shareForm.loading" @click="submitShareLink">
          {{ t("share.generate") }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { useRoute } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { useI18n } from "vue-i18n";
import SideNav from "./components/SideNav.vue";
import TopBar from "./components/TopBar.vue";
import { createShareLink, openExternal } from "./services/api";

const route = useRoute();
const { t } = useI18n();

const pageMeta = computed(() => {
  const routeName = String(route.name || "dashboard");
  return {
    title: t(`route.${routeName}.title`),
    subtitle: t(`route.${routeName}.subtitle`)
  };
});

type ShareQueueItem = {
  path: string;
};

const shareDialogVisible = ref(false);
const shareQueue = ref<ShareQueueItem[]>([]);
const expireOptions = computed(() => [
  { label: t("share.options.forever"), value: "0" },
  { label: t("share.options.day1"), value: "86400" },
  { label: t("share.options.day7"), value: "604800" },
  { label: t("share.options.day30"), value: "2592000" }
]);
const shareForm = reactive({
  localPath: "",
  password: "",
  expire: "604800",
  shareLink: "",
  loading: false,
  error: ""
});

let unlisten: (() => void) | null = null;

const enqueueSharePath = (path: string) => {
  if (!path) {
    return;
  }
  shareQueue.value.push({ path });
  if (!shareDialogVisible.value) {
    openNextShare();
  }
};

const openNextShare = () => {
  const next = shareQueue.value.shift();
  if (!next) {
    return;
  }
  shareForm.localPath = next.path;
  shareForm.password = "";
  shareForm.expire = "604800";
  shareForm.shareLink = "";
  shareForm.error = "";
  shareForm.loading = false;
  shareDialogVisible.value = true;
};

const handleShareDialogClose = () => {
  if (shareQueue.value.length > 0) {
    openNextShare();
  }
};

const submitShareLink = async () => {
  const password = shareForm.password.trim();
  if (password && !/^[a-zA-Z0-9]+$/.test(password)) {
    ElMessage.error(t("share.passwordRule"));
    return;
  }
  shareForm.loading = true;
  shareForm.error = "";
  shareForm.shareLink = "";
  try {
    const expireValue = Number(shareForm.expire);
    const expireSeconds = Number.isFinite(expireValue) && expireValue > 0 ? expireValue : undefined;
    const link = await createShareLink({
      local_path: shareForm.localPath,
      password: password || undefined,
      expire_seconds: expireSeconds
    });
    shareForm.shareLink = link;
    ElMessage.success(t("share.generated"));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    shareForm.error = message;
    ElMessage.error(t("share.createFailed", { msg: message }));
  } finally {
    shareForm.loading = false;
  }
};

const copyShareLink = async () => {
  if (!shareForm.shareLink) {
    return;
  }
  try {
    await navigator.clipboard.writeText(shareForm.shareLink);
    ElMessage.success(t("share.copied"));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    ElMessage.error(t("share.copyFailed", { msg: message }));
  }
};

const openShareLink = async () => {
  if (!shareForm.shareLink) {
    return;
  }
  try {
    await openExternal(shareForm.shareLink);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    ElMessage.error(t("share.openFailed", { msg: message }));
  }
};

onMounted(async () => {
  unlisten = await listen<{ path: string }>("share-request", (event) => {
    enqueueSharePath(event.payload.path);
  });
});

onUnmounted(() => {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
});
</script>

<style scoped>
.share-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.share-path {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.03);
}

.share-label {
  font-size: 12px;
  color: #6b6b6b;
}

.share-value {
  font-size: 13px;
  color: #2a2a2a;
  word-break: break-all;
}

.share-result {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.share-link-row {
  display: grid;
  grid-template-columns: 1fr auto auto;
  gap: 8px;
  align-items: center;
}

.share-error {
  color: #c45656;
  font-size: 13px;
}
</style>
