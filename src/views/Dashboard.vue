<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import TrafficChart from "../components/TrafficChart.vue";
import {
  loadStore,
  stopFrp,
  getFrpStatus,
  onFrpStatus,
  runFrp,
  getAppDataDir,
  writeConfig,
  tunnelsToToml,
  STORE_KEYS,
} from "../api";
import type { TunnelConfig, AppSettings } from "../types";

const props = defineProps<{ tunnels: TunnelConfig[]; settings: AppSettings; theme?: string }>();

const selectedConfig = ref(loadStore<string>("selected_config", ""));
const running = ref(false);
const connecting = ref(false);
const connectError = ref("");

// 流量数据
const MAX_POINTS = 30;
const labels = ref<string[]>([]);
const download = ref<number[]>([]);
const upload = ref<number[]>([]);
const downSpeed = ref(0);
const upSpeed = ref(0);
const totalDown = ref(0);
const totalUp = ref(0);

let timer: number | undefined;
let unlisten: (() => void) | undefined;

function nowLabel() {
  const d = new Date();
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes()
    .toString()
    .padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
}

function initData() {
  for (let i = MAX_POINTS - 1; i >= 0; i--) {
    labels.value.push(`-${i}s`);
    download.value.push(0);
    upload.value.push(0);
  }
}

function tick() {
  if (!running.value) {
    downSpeed.value = 0;
    upSpeed.value = 0;
    return;
  }
  const d = Math.floor(Math.random() * 800 + 200); // 模拟下行 KB/s
  const u = Math.floor(Math.random() * 300 + 50); // 模拟上行 KB/s
  downSpeed.value = d;
  upSpeed.value = u;
  totalDown.value += d;
  totalUp.value += u;

  download.value.push(d);
  upload.value.push(u);
  labels.value.push(nowLabel());
  if (download.value.length > MAX_POINTS) {
    download.value.shift();
    upload.value.shift();
    labels.value.shift();
  }
}

function startSimulation() {
  stopSimulation();
  timer = window.setInterval(tick, 1000);
}

function stopSimulation() {
  if (timer) {
    clearInterval(timer);
    timer = undefined;
  }
}

async function toggleConnection() {
  connecting.value = true;
  connectError.value = "";
  try {
    if (running.value) {
      await stopFrp();
    } else {
      // 生成配置文件并启动真实 frpc
      const frpcPath = loadStore<string>(STORE_KEYS.corePath, "");
      if (!frpcPath) {
        connectError.value = "尚未安装 FRP 核心，请先在「核心下载」中下载";
        return;
      }
      // 生成 frpc.toml
      const toml = await tunnelsToToml({
        serverAddr: props.settings.serverAddr,
        serverPort: props.settings.serverPort,
        user: props.settings.user,
        authToken: props.settings.authToken,
        tunnels: props.tunnels.filter((t) => t.enabled),
      });
      // 写入配置文件
      let configPath = "";
      try {
        const dir = await getAppDataDir();
        configPath = `${dir}\\frpc.toml`;
      } catch {
        configPath = "frpc.toml";
      }
      await writeConfig(configPath, toml);
      await runFrp(frpcPath, configPath);
    }
  } catch (e) {
    connectError.value = String(e);
  } finally {
    connecting.value = false;
  }
}

function fmtBytes(b: number) {
  if (b < 1024) return b + " KB";
  if (b < 1024 * 1024) return (b / 1024).toFixed(1) + " MB";
  return (b / 1024 / 1024).toFixed(2) + " GB";
}

const activeTunnels = computed(() => props.tunnels.filter((t) => t.enabled).length);

onMounted(async () => {
  initData();
  try {
    const status = await getFrpStatus();
    running.value = status?.running ?? false;
  } catch {
    /* 后端未连接时静默 */
  }
  unlisten = await onFrpStatus((data) => {
    running.value = data?.running ?? false;
    if (running.value) startSimulation();
    else stopSimulation();
  });
  if (running.value) startSimulation();
});

onBeforeUnmount(() => {
  stopSimulation();
  unlisten?.();
});
</script>

<template>
  <div class="dashboard">
    <!-- 顶部配置选择与连接 -->
    <div class="card header-card">
      <div class="config-select">
        <span class="muted small">当前配置</span>
        <select class="select" v-model="selectedConfig">
          <option value="">默认配置 (frpc.toml)</option>
          <option v-for="t in tunnels" :key="t.id" :value="t.id">{{ t.name }}</option>
        </select>
      </div>
      <div class="connect-actions">
        <button
          v-if="!running"
          class="btn btn-primary"
          :disabled="connecting"
          @click="toggleConnection"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <path d="M8 5v14l11-7z" />
          </svg>
          {{ connecting ? "连接中..." : "连接" }}
        </button>
        <button
          v-else
          class="btn btn-danger"
          :disabled="connecting"
          @click="toggleConnection"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <rect x="6" y="6" width="12" height="12" rx="2" />
          </svg>
          断开
        </button>
      </div>
      <div class="status-pill" :class="running ? 'on' : 'off'">
        <span class="dot"></span>
        {{ running ? "已连接" : "未连接" }}
      </div>
    </div>

    <div v-if="connectError" class="connect-error">{{ connectError }}</div>

    <!-- 统计卡片 -->
    <div class="stats-grid">
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(52,211,153,.14); color: #34d399">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 11 4 4 4-4" /><path d="M4 21h16" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ downSpeed }} <span class="unit">KB/s</span></div>
          <div class="stat-label">下行速率</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(79,140,255,.14); color: #4f8cff">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 21V9" /><path d="m8 13 4-4 4 4" /><path d="M4 3h16" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ upSpeed }} <span class="unit">KB/s</span></div>
          <div class="stat-label">上行速率</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(251,191,36,.14); color: #fbbf24">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 7h18M3 12h18M3 17h18" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ activeTunnels }}</div>
          <div class="stat-label">活动隧道</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(248,113,113,.14); color: #f87171">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 2v4M12 18v4M4.9 4.9l2.8 2.8M16.3 16.3l2.8 2.8M2 12h4M18 12h4" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ fmtBytes(totalDown) }}</div>
          <div class="stat-label">累计下行</div>
        </div>
      </div>
    </div>

    <!-- 流量图表 -->
    <div class="card chart-card">
      <div class="chart-header flex justify-between items-center">
        <div>
          <h3 class="chart-title">实时流量</h3>
          <span class="muted small">上下行流量实时监控</span>
        </div>
        <div class="flex gap-2">
          <span class="badge" style="background: rgba(52,211,153,.12); color: #34d399">↓ 下行</span>
          <span class="badge" style="background: rgba(79,140,255,.12); color: #4f8cff">↑ 上行</span>
        </div>
      </div>
      <TrafficChart :download="download" :upload="upload" :labels="labels" :theme="theme" />
    </div>

    <!-- 连接信息 -->
    <div class="card info-card">
      <h3 class="chart-title">连接信息</h3>
      <div class="info-grid">
        <div class="info-item">
          <span class="muted small">服务器</span>
          <span class="info-value">{{ settings.serverAddr }}:{{ settings.serverPort }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">协议</span>
          <span class="info-value">{{ settings.transport.toUpperCase() }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">TLS</span>
          <span class="info-value" style="color: var(--success)">{{ settings.tlsEnable ? "已启用" : "未启用" }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">活动隧道</span>
          <span class="info-value">{{ activeTunnels }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 16px;
  animation: fadeUp 0.4s ease;
}

.header-card {
  display: flex;
  align-items: center;
  gap: 24px;
  flex-wrap: wrap;
}
.config-select {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 220px;
  flex: 1;
}
.connect-actions {
  display: flex;
  gap: 10px;
}
.status-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 600;
}
.status-pill .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.status-pill.on {
  background: rgba(52, 211, 153, 0.12);
  color: var(--success);
}
.status-pill.on .dot {
  background: var(--success);
  box-shadow: 0 0 8px var(--success);
  animation: pulse 1.5s infinite;
}
.status-pill.off {
  background: rgba(107, 114, 128, 0.15);
  color: var(--text-muted);
}
.status-pill.off .dot {
  background: var(--text-muted);
}
.connect-error {
  background: rgba(248, 113, 113, 0.12);
  border: 1px solid rgba(248, 113, 113, 0.25);
  color: var(--danger);
  padding: 10px 16px;
  border-radius: var(--radius-sm);
  font-size: 13px;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
}
.stat-card {
  display: flex;
  align-items: center;
  gap: 14px;
}
.stat-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.stat-value {
  font-size: 22px;
  font-weight: 700;
  line-height: 1.2;
}
.stat-value .unit {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted);
}
.stat-label {
  font-size: 12px;
  color: var(--text-secondary);
}

.chart-card {
  flex: 1;
  min-height: 320px;
  display: flex;
  flex-direction: column;
}
.chart-header {
  margin-bottom: 12px;
}
.chart-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 2px;
}
.chart-card :deep(.chart) {
  flex: 1;
}

.info-card .info-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 16px;
  margin-top: 12px;
}
.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.info-value {
  font-size: 14px;
  font-weight: 500;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
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
</style>
