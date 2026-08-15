<script setup lang="ts">
import { ref, computed } from "vue";
import { FRP_VERSIONS, loadStore, saveStore, STORE_KEYS, downloadFrp } from "../api";
import type { FrpPlatform } from "../types";

const versions = FRP_VERSIONS;
const selectedVersion = ref(loadStore<string>("frp_version", versions[0].tag));

const os = ref<FrpPlatform["os"]>("windows");
const arch = ref<FrpPlatform["arch"]>("amd64");

const downloading = ref(false);
const progress = ref(0);
const downloadError = ref("");
const installed = ref(loadStore<string>(STORE_KEYS.corePath, ""));
const downloadedVersion = ref(loadStore<string>("frp_downloaded_version", ""));

const osOptions: { value: FrpPlatform["os"]; label: string }[] = [
  { value: "windows", label: "Windows" },
  { value: "linux", label: "Linux" },
  { value: "darwin", label: "macOS" },
  { value: "freebsd", label: "FreeBSD" },
];
const archOptions: { value: FrpPlatform["arch"]; label: string }[] = [
  { value: "amd64", label: "amd64 (x86_64)" },
  { value: "arm64", label: "arm64" },
  { value: "386", label: "386" },
  { value: "arm", label: "arm" },
];

function selectVersion(tag: string) {
  selectedVersion.value = tag;
  saveStore("frp_version", tag);
}

const fileName = computed(() => {
  const ext = os.value === "windows" ? ".zip" : ".tar.gz";
  return `frp_${selectedVersion.value.slice(1)}_${os.value}_${arch.value}${ext}`;
});

const downloadUrl = computed(
  () =>
    `https://github.com/fatedier/frp/releases/download/${selectedVersion.value}/${fileName.value}`
);

async function startDownload() {
  if (downloading.value) return;
  downloading.value = true;
  progress.value = 10;
  downloadError.value = "";
  try {
    const result = await downloadFrp(selectedVersion.value, os.value, arch.value);
    progress.value = 100;
    installed.value = result.path;
    downloadedVersion.value = result.version;
    saveStore(STORE_KEYS.corePath, installed.value);
    saveStore("frp_downloaded_version", downloadedVersion.value);
  } catch (e) {
    downloadError.value = String(e);
  } finally {
    setTimeout(() => {
      downloading.value = false;
    }, 300);
  }
}

function openReleasePage() {
  window.open(downloadUrl.value, "_blank");
}
</script>

<template>
  <div class="core-download">
    <div class="page-header">
      <h2>核心下载</h2>
      <p class="muted small">下载 FRP 核心程序，选择版本与平台架构</p>
    </div>

    <div class="layout">
      <!-- 左侧：版本选择 -->
      <div class="card versions-card">
        <h3 class="section-title">选择版本</h3>
        <div class="version-list">
          <button
            v-for="v in versions"
            :key="v.tag"
            class="version-item"
            :class="{ active: selectedVersion === v.tag }"
            @click="selectVersion(v.tag)"
          >
            <div class="version-tag">{{ v.tag }}</div>
            <div class="muted small">{{ v.publishedAt }}</div>
            <span v-if="v.tag === versions[0].tag" class="badge latest">最新</span>
          </button>
        </div>
      </div>

      <!-- 右侧：下载配置 -->
      <div class="right-col">
        <div class="card">
          <h3 class="section-title">平台与架构</h3>
          <div class="platform-grid">
            <div class="field">
              <label>操作系统</label>
              <select class="select" v-model="os">
                <option v-for="o in osOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
              </select>
            </div>
            <div class="field">
              <label>架构</label>
              <select class="select" v-model="arch">
                <option v-for="a in archOptions" :key="a.value" :value="a.value">{{ a.label }}</option>
              </select>
            </div>
          </div>

          <div class="file-preview">
            <div class="file-name">
              <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <path d="M14 2v6h6" />
              </svg>
              {{ fileName }}
            </div>
            <div class="muted small">下载地址：{{ downloadUrl }}</div>
          </div>

          <div class="download-actions">
            <button class="btn btn-primary" :disabled="downloading" @click="startDownload">
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
              </svg>
              {{ downloading ? "下载中..." : "下载" }}
            </button>
            <button class="btn btn-ghost" @click="openReleasePage">前往 GitHub Releases</button>
          </div>

          <div v-if="downloading || progress > 0" class="progress-wrap">
            <div class="progress-bar">
              <div class="progress-fill" :style="{ width: progress + '%' }"></div>
            </div>
            <span class="small muted">{{ Math.floor(progress) }}%</span>
          </div>
          <p v-if="downloadError" class="error-text">{{ downloadError }}</p>
        </div>

        <div class="card">
          <h3 class="section-title">已安装核心</h3>
          <div v-if="installed" class="installed">
            <div class="installed-icon">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <path d="M22 4 12 14.01l-3-3" />
              </svg>
            </div>
            <div>
              <div class="installed-path">{{ installed }}</div>
              <div class="muted small">版本：{{ downloadedVersion }}</div>
            </div>
          </div>
          <div v-else class="muted small">尚未安装 FRP 核心程序</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.core-download {
  animation: fadeUp 0.4s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.layout {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 16px;
  margin-top: 20px;
}
.section-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 14px;
}
.version-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.version-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  color: var(--text);
  transition: all var(--transition);
  text-align: left;
}
.version-item:hover {
  border-color: var(--primary);
}
.version-item.active {
  border-color: var(--primary);
  background: var(--primary-soft);
}
.version-tag {
  font-weight: 600;
  font-family: monospace;
  flex: 1;
}
.latest {
  background: rgba(52, 211, 153, 0.15);
  color: var(--success);
}
.right-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.platform-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}
.file-preview {
  margin-top: 18px;
  padding: 14px;
  background: var(--bg-elevated);
  border: 1px dashed var(--border-light);
  border-radius: var(--radius-sm);
}
.file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: monospace;
  font-size: 13px;
  color: var(--text);
  word-break: break-all;
}
.download-actions {
  display: flex;
  gap: 10px;
  margin-top: 18px;
  flex-wrap: wrap;
}
.progress-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 14px;
}
.progress-bar {
  flex: 1;
  height: 8px;
  background: var(--bg-elevated);
  border-radius: 4px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--primary), var(--success));
  border-radius: 4px;
  transition: width 0.2s ease;
}
.error-text {
  color: var(--danger);
  font-size: 12px;
  margin-top: 8px;
  word-break: break-all;
}
.installed {
  display: flex;
  align-items: center;
  gap: 12px;
}
.installed-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  background: rgba(52, 211, 153, 0.14);
  color: var(--success);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.installed-path {
  font-family: monospace;
  font-size: 13px;
  word-break: break-all;
}
@keyframes fadeUp {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

/* 响应式适配 */
@media (max-width: 860px) {
  .layout {
    grid-template-columns: 1fr;
  }
  .version-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  }
}
@media (max-width: 560px) {
  .platform-grid {
    grid-template-columns: 1fr;
  }
}
</style>
