<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import Spinner from "../components/Spinner.vue";
import Skeleton from "../components/Skeleton.vue";
import { toast } from "../composables/toast";
import {
  loadStore,
  saveStore,
  STORE_KEYS,
  detectPlatform,
  listFrpReleases,
  downloadFrp,
  verifyCore,
  revealPath,
  onCoreDownload,
  fmtBytes,
  FRP_VERSION_FALLBACK,
} from "../api";
import type { CoreProgress, InstalledCore, ReleaseInfo } from "../types";

const os = ref("windows");
const arch = ref("amd64");
const osLocked = ref(false);
const archLocked = ref(false);

// 先展示加载动画，不再用内置版本直接冒充真实数据
const releases = ref<ReleaseInfo[]>([]);
const selected = ref(loadStore<string>(STORE_KEYS.frpVersion, FRP_VERSION_FALLBACK[0].tag));
const installed = ref<InstalledCore | null>(null);
const downloading = ref(false);
const verifying = ref(false);
const progress = ref<CoreProgress | null>(null);
const error = ref("");
const notice = ref("");
const source = ref<"github" | "builtin" | "loading">("loading");
const loadingReleases = ref(true);
const loadingPlatform = ref(true);

const osOptions = [
  { value: "windows", label: "Windows" },
  { value: "linux", label: "Linux" },
  { value: "darwin", label: "macOS" },
  { value: "freebsd", label: "FreeBSD" },
];
const archOptions = [
  { value: "amd64", label: "amd64 (x86_64)" },
  { value: "arm64", label: "arm64" },
  { value: "386", label: "386" },
  { value: "arm", label: "arm" },
];

const fileName = computed(() => {
  const ext = os.value === "windows" ? ".zip" : ".tar.gz";
  return `frp_${selected.value.replace(/^v/, "")}_${os.value}_${arch.value}${ext}`;
});

const downloadUrl = computed(
  () => `https://github.com/fatedier/frp/releases/download/${selected.value}/${fileName.value}`
);

const percent = computed(() => {
  if (!progress.value) return 0;
  if (progress.value.stage === "done") return 100;
  return Math.floor(progress.value.percent || 0);
});

const stageText = computed(() => {
  switch (progress.value?.stage) {
    case "download":
      return "下载中";
    case "retry":
      return "切换下载源";
    case "extract":
      return "解压中";
    case "done":
      return "安装完成";
    case "error":
      return "下载失败";
    default:
      return "";
  }
});

function selectVersion(tag: string) {
  selected.value = tag;
  saveStore(STORE_KEYS.frpVersion, tag);
}

async function loadPlatform() {
  loadingPlatform.value = true;
  try {
    const p = await detectPlatform();
    // 默认对齐当前运行平台，避免下载到无法执行的二进制
    os.value = p.os;
    arch.value = p.arch;
    osLocked.value = true;
    archLocked.value = true;
    installed.value = p.installed;
  } catch (e) {
    console.error(e);
    error.value = "平台识别失败，请手动选择操作系统与架构";
  } finally {
    loadingPlatform.value = false;
  }
}

async function loadReleases() {
  loadingReleases.value = true;
  source.value = "loading";
  try {
    const list = await listFrpReleases(8);
    if (list.length) {
      releases.value = list;
      source.value = "github";
      if (!releases.value.some((r) => r.tag === selected.value)) {
        selected.value = list[0].tag;
      }
    } else {
      throw new Error("GitHub 未返回可用版本");
    }
  } catch (e) {
    console.warn("读取 GitHub 版本列表失败，使用内置回退列表", e);
    releases.value = FRP_VERSION_FALLBACK;
    source.value = "builtin";
    if (!releases.value.some((r) => r.tag === selected.value)) {
      selected.value = releases.value[0].tag;
    }
    notice.value = "GitHub 版本列表获取失败，已切换为内置版本列表（仍可正常下载）";
  } finally {
    loadingReleases.value = false;
  }
}

async function startDownload() {
  if (downloading.value) return;
  downloading.value = true;
  error.value = "";
  notice.value = "";
  progress.value = { stage: "download", message: "准备下载...", downloaded: 0, total: 0, percent: 0 };
  try {
    const result = await downloadFrp(selected.value, os.value, arch.value);
    installed.value = {
      path: result.path,
      version: result.version,
      os: result.os,
      arch: result.arch,
      installedAt: Date.now(),
      size: result.size,
    };
    notice.value = `frpc ${result.version} 安装完成`;
    progress.value = { stage: "done", message: notice.value, downloaded: 1, total: 1, percent: 100 };
  } catch (e) {
    error.value = typeof e === "string" ? e : String(e);
    progress.value = { stage: "error", message: error.value, downloaded: 0, total: 0, percent: 0 };
  } finally {
    downloading.value = false;
  }
}

async function doVerify() {
  if (!installed.value || verifying.value) return;
  verifying.value = true;
  try {
    const v = await verifyCore(installed.value.path);
    notice.value = `核心可执行，版本 ${v}`;
    error.value = "";
  } catch (e) {
    error.value = `核心校验失败: ${String(e)}`;
  } finally {
    verifying.value = false;
  }
}

async function openDataDir() {
  try {
    await revealPath(installed.value?.path || "");
  } catch (e) {
    error.value = String(e);
  }
}

let unlisten: (() => void) | undefined;

onMounted(async () => {
  await loadPlatform();
  await loadReleases();
  unlisten = await onCoreDownload((p) => {
    progress.value = p;
  });
});

watch(notice, (v) => {
  if (v) toast.success(v);
});
watch(error, (v) => {
  if (v) toast.error("操作失败", v);
});

onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <div class="core-download">
    <div class="page-header">
      <h2>核心下载</h2>
      <p class="muted small">
        从 GitHub Releases 下载 frpc 可执行文件，默认对齐当前运行平台
        <span class="badge src-badge">
          {{ source === "loading" ? "正在读取版本" : source === "github" ? "版本来自 GitHub" : "内置版本列表" }}
        </span>
      </p>
    </div>

    <div class="layout">
      <!-- 版本 -->
      <div class="card versions-card">
        <h3 class="section-title">选择版本</h3>
        <div v-if="loadingReleases" class="version-list">
          <div v-for="n in 6" :key="`sk-ver-${n}`" class="version-skeleton">
            <Skeleton width="72px" height="14px" />
            <Skeleton width="110px" height="10px" />
          </div>
          <div class="sk-inline muted small">
            <Spinner :size="13" :width="2" />
            正在从 GitHub 读取版本列表…
          </div>
        </div>
        <div v-else class="version-list">
          <button
            v-for="(v, i) in releases"
            :key="v.tag"
            class="version-item"
            :class="{ active: selected === v.tag }"
            @click="selectVersion(v.tag)"
          >
            <div class="version-tag">{{ v.tag }}</div>
            <div class="muted small">{{ v.publishedAt }}</div>
            <span v-if="i === 0" class="badge latest">最新</span>
            <span v-if="installed?.version?.includes(v.version)" class="badge installed-badge">已装</span>
          </button>
        </div>
      </div>

      <!-- 下载 -->
      <div class="right-col">
        <div class="card">
          <h3 class="section-title">平台与架构</h3>
          <div v-if="loadingPlatform" class="platform-grid">
            <div class="field">
              <Skeleton width="90px" height="11px" />
              <Skeleton width="100%" height="38px" radius="8px" />
            </div>
            <div class="field">
              <Skeleton width="70px" height="11px" />
              <Skeleton width="100%" height="38px" radius="8px" />
            </div>
            <p class="muted small sk-inline sk-platform">
              <Spinner :size="12" :width="2" />
              正在识别当前操作系统与架构…
            </p>
          </div>
          <div v-else class="platform-grid">
            <div class="field">
              <label>操作系统{{ osLocked ? "（已自动识别）" : "" }}</label>
              <select class="select" v-model="os">
                <option v-for="o in osOptions" :key="o.value" :value="o.value">{{ o.label }}</option>
              </select>
            </div>
            <div class="field">
              <label>架构{{ archLocked ? "（已自动识别）" : "" }}</label>
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
            <div class="muted small url-line">{{ downloadUrl }}</div>
          </div>

          <div class="download-actions">
            <button class="btn btn-primary" :disabled="downloading || loadingPlatform || loadingReleases" @click="startDownload">
              <Spinner v-if="downloading" :size="15" :width="2" />
              <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
              </svg>
              {{ downloading ? "下载中..." : installed ? "重新下载 / 更换版本" : "下载并安装" }}
            </button>
            <button class="btn btn-ghost" @click="openDataDir">打开核心目录</button>
          </div>

          <div v-if="progress && (downloading || progress.stage === 'done' || progress.stage === 'error')" class="progress-wrap">
            <div class="progress-bar">
              <div
                class="progress-fill"
                :class="{ fail: progress.stage === 'error' }"
                :style="{ width: percent + '%' }"
              ></div>
            </div>
            <span class="small muted progress-text">
              {{ stageText }} {{ percent }}%
              <template v-if="progress.total > 0">
                · {{ fmtBytes(progress.downloaded) }} / {{ fmtBytes(progress.total) }}
              </template>
            </span>
          </div>
          <p v-if="progress?.message && downloading" class="muted small mt-2">{{ progress.message }}</p>
          <p v-if="error" class="error-text">{{ error }}</p>
          <p v-if="notice && !error" class="ok-text">{{ notice }}</p>
        </div>

        <div class="card">
          <h3 class="section-title">已安装核心</h3>
          <div v-if="loadingPlatform" class="installed">
            <Skeleton width="40px" height="40px" radius="10px" />
            <div class="installed-main">
              <Skeleton width="70%" height="12px" />
              <Skeleton width="45%" height="10px" />
            </div>
          </div>
          <div v-else-if="installed" class="installed">
            <div class="installed-icon">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <path d="M22 4 12 14.01l-3-3" />
              </svg>
            </div>
            <div class="installed-main">
              <div class="installed-path mono">{{ installed.path }}</div>
              <div class="muted small">
                版本 {{ installed.version }} · {{ installed.os }}/{{ installed.arch }} · {{ fmtBytes(installed.size) }}
              </div>
            </div>
            <button class="btn btn-ghost btn-sm" :disabled="verifying" @click="doVerify">
              <Spinner v-if="verifying" :size="12" :width="2" />
              {{ verifying ? "校验中" : "校验" }}
            </button>
          </div>
          <div v-else class="muted small">尚未安装 frpc 核心，下载后即可在控制面板启动内网穿透</div>
        </div>

        <div class="card tip-card">
          <h3 class="section-title">穿透原理</h3>
          <ol class="tips">
            <li>本应用按「配置管理」中的配置生成 frpc.toml（含服务器地址、令牌、隧道列表）。</li>
            <li>启动时先为每条 TCP 类隧道开启本地计量网关，frpc 的 localPort 指向网关端口。</li>
            <li>frpc 连接 frps 建立隧道，外网访问流量经网关转发到真实本地服务并逐字节统计。</li>
          </ol>
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
.src-badge {
  background: var(--bg-hover);
  color: var(--text-muted);
  margin-left: 6px;
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
  gap: 8px;
  padding: 10px 12px;
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
.version-skeleton {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  border: 1px solid var(--border);
}
.sk-inline {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.sk-platform {
  grid-column: 1 / -1;
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
.installed-badge {
  background: rgba(79, 140, 255, 0.15);
  color: var(--primary);
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
  word-break: break-all;
}
.url-line {
  margin-top: 6px;
  font-family: monospace;
  word-break: break-all;
  opacity: 0.7;
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
.progress-fill.fail {
  background: var(--danger);
}
.progress-text {
  white-space: nowrap;
}
.error-text {
  color: var(--danger);
  font-size: 12px;
  margin-top: 8px;
  word-break: break-all;
}
.ok-text {
  color: var(--success);
  font-size: 12px;
  margin-top: 8px;
}
.installed {
  display: flex;
  align-items: center;
  gap: 12px;
}
.installed-main {
  flex: 1;
  min-width: 0;
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
  font-size: 12px;
  word-break: break-all;
}
.mono {
  font-family: "JetBrains Mono", "Fira Code", monospace;
}
.btn-sm {
  padding: 5px 10px;
  font-size: 12px;
}
.tips {
  margin: 0;
  padding-left: 18px;
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.9;
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

