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
      title="创建分享链接"
      width="520px"
      @close="handleShareDialogClose"
    >
      <div class="share-form">
        <div class="share-path">
          <div class="share-label">本地路径</div>
          <div class="share-value">{{ shareForm.localPath }}</div>
        </div>
        <el-form :model="shareForm" label-width="88px">
          <el-form-item label="提取密码">
            <el-input
              v-model="shareForm.password"
              placeholder="留空表示无密码"
              maxlength="32"
              show-word-limit
            />
          </el-form-item>
          <el-form-item label="有效期">
            <el-select v-model="shareForm.expire" placeholder="请选择">
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
          <div class="share-label">分享链接</div>
          <div class="share-link-row">
            <el-input v-model="shareForm.shareLink" readonly />
            <el-button @click="copyShareLink">复制</el-button>
            <el-button type="primary" @click="openShareLink">打开</el-button>
          </div>
        </div>
        <div v-if="shareForm.error" class="share-error">{{ shareForm.error }}</div>
      </div>
      <template #footer>
        <el-button @click="shareDialogVisible = false">关闭</el-button>
        <el-button type="primary" :loading="shareForm.loading" @click="submitShareLink">
          生成
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
import SideNav from "./components/SideNav.vue";
import TopBar from "./components/TopBar.vue";
import { createShareLink, openExternal } from "./services/api";

const route = useRoute();

const pageMeta = computed(() => {
  return {
    title: (route.meta.title as string) || "概览",
    subtitle: (route.meta.subtitle as string) || "同步状态与活动总览"
  };
});

type ShareQueueItem = {
  path: string;
};

const shareDialogVisible = ref(false);
const shareQueue = ref<ShareQueueItem[]>([]);
const expireOptions = [
  { label: "永久", value: "0" },
  { label: "1 天", value: "86400" },
  { label: "7 天", value: "604800" },
  { label: "30 天", value: "2592000" }
];
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
    ElMessage.error("提取密码仅支持字母和数字");
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
    ElMessage.success("分享链接已生成");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    shareForm.error = message;
    ElMessage.error(`创建分享链接失败: ${message}`);
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
    ElMessage.success("已复制分享链接");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    ElMessage.error(`复制失败: ${message}`);
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
    ElMessage.error(`打开失败: ${message}`);
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
