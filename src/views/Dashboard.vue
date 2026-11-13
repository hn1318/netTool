<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import TrafficChart from "../components/TrafficChart.vue";
import Spinner from "../components/Spinner.vue";
import Skeleton from "../components/Skeleton.vue";
import EmptyState from "../components/EmptyState.vue";
import { toast } from "../composables/toast";
import {
  EMPTY_SNAPSHOT,
  fmtBytes,
  fmtSpeed,
  startFrpc,
  stopFrpc,
  getFrpStatus,
  getTraffic,
  resetTraffic,
  onFrpStatus,
  onFrpTraffic,
  tunnelAccessUrl,
} from "../api";
import type { Profile, StartResult, TrafficSnapshot, TunnelConfig } from "../types";

const props = defineProps<{ profile: Profile; theme?: string }>();

const snapshot = ref<TrafficSnapshot>({ ...EMPTY_SNAPSHOT });
const connecting = ref(false);
const connectError = ref("");
const runtime = ref<StartResult | null>(null);
const loading = ref(true);
const nowTick = ref(Date.now());

const MAX_POINTS = 60;
const labels = ref<string[]>([]);
const download = ref<number[]>([]);
const upload = ref<number[]>([]);

let unlistenTraffic: (() => void) | undefined;
let unlistenStatus: (() => void) | undefined;

const running = computed(() => snapshot.value.running);

const enabledTunnels = computed(() => props.profile.tunnels.filter((t) => t.enabled));

/** 合并运行快照与配置，保证 UDP 等无字节统计的隧道也能展示 */
const rows = computed(() => {
  const map = new Map(snapshot.value.tunnels.map((t) => [t.name, t]));
  return enabledTunnels.value.map((t) => {
    const hit = map.get(t.name);
    return (
      hit || {
        id: t.id,
        name: t.name,
        type: t.type,
        target: `${t.localIp}:${t.localPort}`,
        gatewayPort: 0,
        upSpeed: 0,
        downSpeed: 0,
        upTotal: 0,
        downTotal: 0,
        conns: 0,
        connTotal: 0,
        status: "",
        err: "",
        remoteAddr: "",
      }
    );
  });
});

function nowLabel() {
  const d = new Date();
  const p = (n: number) => n.toString().padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function pushPoint(down: number, up: number) {
  download.value.push(Math.round(down / 1024));
  upload.value.push(Math.round(up / 1024));
  labels.value.push(nowLabel());
  while (download.value.length > MAX_POINTS) {
    download.value.shift();
    upload.value.shift();
    labels.value.shift();
  }
}

function applySnapshot(s: TrafficSnapshot) {
  const wasRunning = running.value;
  snapshot.value = s;
  if (!s.running) {
    if (wasRunning) {
      download.value = download.value.map(() => 0);
      upload.value = upload.value.map(() => 0);
    }
    return;
  }
  pushPoint(s.downSpeed, s.upSpeed);
}

async function toggleConnection() {
  connecting.value = true;
  connectError.value = "";
  try {
    if (running.value) {
      await stopFrpc();
      runtime.value = null;
      snapshot.value = { ...EMPTY_SNAPSHOT };
      toast.info("已断开连接");
    } else {
      const res = await startFrpc(props.profile);
      runtime.value = res;
      connectError.value = "";
      toast.success("已开始连接", `${res.profileName} · ${res.gateways.length} 个统计网关`);
    }
  } catch (e) {
    connectError.value = typeof e === "string" ? e : String(e);
    toast.error("连接失败", connectError.value);
  } finally {
    connecting.value = false;
  }
}

async function doReset() {
  try {
    await resetTraffic();
    download.value = download.value.map(() => 0);
    upload.value = upload.value.map(() => 0);
    snapshot.value = { ...snapshot.value, upTotal: 0, downTotal: 0, upSpeed: 0, downSpeed: 0 };
    toast.success("统计已重置");
  } catch (e) {
    connectError.value = String(e);
  }
}

function statusLabel(row: { status: string; err: string; gatewayPort: number }) {
  if (row.err) return "异常";
  if (row.status === "running") return "运行中";
  if (row.status) return row.status;
  if (row.gatewayPort === 0) return "无字节统计";
  return "等待回报";
}

function statusClass(row: { status: string; err: string }) {
  if (row.err) return "err";
  if (row.status === "running") return "ok";
  return "idle";
}

function remoteText(row: { remoteAddr: string; type: string }, tunnel?: TunnelConfig) {
  if (row.remoteAddr) return row.remoteAddr;
  if (!tunnel) return "-";
  if (tunnel.type === "http" || tunnel.type === "https") {
    return tunnelAccessUrl(tunnel) || "未配置域名";
  }
  if (tunnel.type === "tcp" || tunnel.type === "udp") {
    return tunnel.remotePort ? `:${tunnel.remotePort}` : "-";
  }
  return "密钥隧道";
}

const uptime = computed(() => {
  if (!snapshot.value.running || !snapshot.value.startedAt) return "-";
  const secs = Math.max(0, Math.floor((nowTick.value - snapshot.value.startedAt) / 1000));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return h > 0 ? `${h}h ${m}m ${s}s` : m > 0 ? `${m}m ${s}s` : `${s}s`;
});

let tickTimer: number | undefined;

onMounted(async () => {
  for (let i = 0; i < 30; i++) {
    labels.value.push("");
    download.value.push(0);
    upload.value.push(0);
  }
  try {
    const [status, snap] = await Promise.all([getFrpStatus(), getTraffic()]);
    if (status?.running && snap?.running) {
      snapshot.value = snap;
      runtime.value = {
        pid: status.pid,
        profileId: status.profileId,
        profileName: status.profileName,
        configPath: status.configPath,
        webPort: status.webPort,
        gateways: [],
      };
    }
  } catch (e) {
    console.warn("读取运行状态失败", e);
  } finally {
    loading.value = false;
  }

  tickTimer = window.setInterval(() => (nowTick.value = Date.now()), 1000);

  unlistenTraffic = await onFrpTraffic((s) => applySnapshot(s));
  unlistenStatus = await onFrpStatus((s) => {
    if (!s.running) {
      snapshot.value = { ...snapshot.value, running: false, upSpeed: 0, downSpeed: 0 };
      runtime.value = null;
    }
  });
});

onBeforeUnmount(() => {
  unlistenTraffic?.();
  unlistenStatus?.();
  if (tickTimer) clearInterval(tickTimer);
});
</script>

<template>
  <div class="dashboard">
    <!-- 顶部状态栏 -->
    <div class="card header-card">
      <div class="config-info">
        <span class="muted small">当前配置</span>
        <div class="config-name">
          {{ profile.name }}
          <span class="badge transport-badge">{{ profile.transport.toUpperCase() }}</span>
        </div>
        <span class="muted small mono">{{ profile.serverAddr || "未设置服务器" }}:{{ profile.serverPort }}</span>
      </div>
      <div class="connect-actions">
        <button v-if="!running" class="btn btn-primary" :disabled="connecting || loading" @click="toggleConnection">
          <Spinner v-if="connecting" :size="14" :width="2" />
          <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <path d="M8 5v14l11-7z" />
          </svg>
          {{ connecting ? "连接中..." : "连接" }}
        </button>
        <button v-else class="btn btn-danger" :disabled="connecting || loading" @click="toggleConnection">
          <Spinner v-if="connecting" :size="14" :width="2" />
          <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
            <rect x="6" y="6" width="12" height="12" rx="2" />
          </svg>
          {{ connecting ? "断开中..." : "断开" }}
        </button>
        <button class="btn btn-ghost" :disabled="!running || loading" @click="doReset">重置统计</button>
      </div>
      <div class="status-pill" :class="loading ? 'off' : running ? 'on' : 'off'">
        <span class="dot"></span>
        {{ loading ? "读取中" : running ? "已连接" : "未连接" }}
      </div>
    </div>

    <div v-if="connectError" class="connect-error">{{ connectError }}</div>

    <!-- 初次读取运行状态时的加载动画（占位骨架，不展示假的统计值） -->
    <template v-if="loading">
      <div class="stats-grid">
        <div v-for="n in 4" :key="`sk-stat-${n}`" class="card stat-card">
          <Skeleton width="44px" height="44px" radius="10px" />
          <div class="sk-stat-main">
            <Skeleton width="92px" height="20px" />
            <Skeleton width="150px" height="10px" />
          </div>
        </div>
      </div>

      <div class="card chart-card">
        <div class="chart-header">
          <Skeleton width="120px" height="16px" />
          <Skeleton width="220px" height="10px" />
        </div>
        <div class="sk-chart">
          <Skeleton width="100%" height="100%" radius="10px" />
        </div>
      </div>

      <div class="card table-card">
        <div class="sk-table-head">
          <Skeleton width="130px" height="16px" />
          <Skeleton width="180px" height="10px" />
        </div>
        <div v-for="n in 3" :key="`sk-row-${n}`" class="sk-table-row">
          <Skeleton width="26%" height="12px" />
          <Skeleton width="12%" height="12px" />
          <Skeleton width="30%" height="12px" />
          <Skeleton width="10%" height="12px" />
          <Skeleton width="10%" height="12px" />
        </div>
      </div>
    </template>

    <!-- 统计卡片 -->
    <div v-else class="stats-grid">
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(52,211,153,.14); color: #34d399">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 11 4 4 4-4" /><path d="M4 21h16" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ fmtSpeed(snapshot.downSpeed) }}</div>
          <div class="stat-label">下行速率 · 累计 {{ fmtBytes(snapshot.downTotal) }}</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(79,140,255,.14); color: #4f8cff">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 21V9" /><path d="m8 13 4-4 4 4" /><path d="M4 3h16" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ fmtSpeed(snapshot.upSpeed) }}</div>
          <div class="stat-label">上行速率 · 累计 {{ fmtBytes(snapshot.upTotal) }}</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(251,191,36,.14); color: #fbbf24">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3" /><path d="M12 2v4M12 18v4M2 12h4M18 12h4" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ snapshot.conns }}</div>
          <div class="stat-label">并发连接 · {{ rows.length }} 条隧道</div>
        </div>
      </div>
      <div class="card stat-card">
        <div class="stat-icon" style="background: rgba(167,139,250,.14); color: #a78bfa">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" />
          </svg>
        </div>
        <div>
          <div class="stat-value">{{ uptime }}</div>
          <div class="stat-label">本次连接时长</div>
        </div>
      </div>
    </div>

    <!-- 实时流量 -->
    <div v-if="!loading" class="card chart-card">
      <div class="chart-header flex justify-between items-center">
        <div>
          <h3 class="chart-title">实时流量</h3>
          <span class="muted small">单位 KB/s，由本地计量网关逐秒采样真实字节数</span>
        </div>
        <div class="flex gap-2">
          <span class="badge" style="background: rgba(52,211,153,.12); color: #34d399">↓ 下行</span>
          <span class="badge" style="background: rgba(79,140,255,.12); color: #4f8cff">↑ 上行</span>
        </div>
      </div>
      <TrafficChart :download="download" :upload="upload" :labels="labels" :theme="theme" />
    </div>

    <!-- 隧道实时状态 -->
    <div v-if="!loading" class="card table-card">
      <div class="flex justify-between items-center" style="margin-bottom: 12px">
        <h3 class="chart-title">隧道流量明细</h3>
        <span class="muted small">状态来自 frpc 管理接口 /api/status</span>
      </div>
      <div class="table-wrap">
        <table class="tunnel-table">
          <thead>
            <tr>
              <th>隧道</th>
              <th>类型</th>
              <th>本地 → 远端</th>
              <th>下行</th>
              <th>上行</th>
              <th>连接</th>
              <th>状态</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in rows" :key="row.id">
              <td class="name">{{ row.name }}</td>
              <td><span class="badge type-badge">{{ row.type.toUpperCase() }}</span></td>
              <td class="mono small">
                {{ row.target }}
                <span class="muted">→</span>
                {{ remoteText(row, enabledTunnels.find((t) => t.name === row.name)) }}
              </td>
              <td>
                <div class="speed down">{{ fmtSpeed(row.downSpeed) }}</div>
                <div class="muted small">{{ fmtBytes(row.downTotal) }}</div>
              </td>
              <td>
                <div class="speed up">{{ fmtSpeed(row.upSpeed) }}</div>
                <div class="muted small">{{ fmtBytes(row.upTotal) }}</div>
              </td>
              <td>{{ row.conns }}<span class="muted small"> / {{ row.connTotal }}</span></td>
              <td>
                <span class="status-tag" :class="statusClass(row)">{{ statusLabel(row) }}</span>
                <div v-if="row.err" class="err-text">{{ row.err }}</div>
              </td>
            </tr>
            <tr v-if="rows.length === 0">
              <td colspan="7">
                <EmptyState
                  compact
                  title="当前配置没有启用的隧道"
                  description="到「隧道管理」启用或新增隧道后，回到这里重新连接即可看到真实流量"
                />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 运行信息 -->
    <div class="card info-card" v-if="running && !loading">
      <h3 class="chart-title">运行信息</h3>
      <div class="info-grid">
        <div class="info-item">
          <span class="muted small">进程 PID</span>
          <span class="info-value mono">{{ runtime?.pid || "-" }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">配置文件</span>
          <span class="info-value mono small">{{ runtime?.configPath || "-" }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">管理接口</span>
          <span class="info-value mono">127.0.0.1:{{ runtime?.webPort || "-" }}</span>
        </div>
        <div class="info-item">
          <span class="muted small">统计网关</span>
          <span class="info-value mono small">
            {{ rows.filter((r) => r.gatewayPort > 0).length }} 个端口转发中
          </span>
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
.config-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 220px;
}
.config-name {
  font-size: 16px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 8px;
}
.transport-badge {
  background: var(--bg-hover);
  color: var(--text-muted);
}
.mono {
  font-family: "JetBrains Mono", "Fira Code", monospace;
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
  word-break: break-all;
}
.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
  gap: 16px;
}
.stat-card {
  display: flex;
  align-items: center;
  gap: 14px;
}
.sk-stat-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 9px;
}
.sk-chart {
  flex: 1;
  min-height: 230px;
  display: flex;
}
.sk-table-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
}
.sk-table-row {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 12px 0;
  border-bottom: 1px solid var(--border);
}
.sk-table-row:last-child {
  border-bottom: none;
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
  font-size: 21px;
  font-weight: 700;
  line-height: 1.2;
}
.stat-label {
  font-size: 12px;
  color: var(--text-secondary);
}
.chart-card {
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
.table-card {
  padding-bottom: 8px;
}
.table-wrap {
  overflow-x: auto;
}
.tunnel-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.tunnel-table th {
  text-align: left;
  font-weight: 500;
  font-size: 12px;
  color: var(--text-muted);
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}
.tunnel-table td {
  padding: 10px;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}
.tunnel-table tr:last-child td {
  border-bottom: none;
}
.tunnel-table .name {
  font-weight: 600;
}
.type-badge {
  background: var(--bg-hover);
  color: var(--text-secondary);
}
.speed {
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-weight: 600;
}
.speed.down {
  color: var(--success);
}
.speed.up {
  color: var(--primary);
}
.status-tag {
  display: inline-flex;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}
.status-tag.ok {
  background: rgba(52, 211, 153, 0.12);
  color: var(--success);
}
.status-tag.err {
  background: rgba(248, 113, 113, 0.12);
  color: var(--danger);
}
.status-tag.idle {
  background: var(--bg-hover);
  color: var(--text-muted);
}
.err-text {
  font-size: 11px;
  color: var(--danger);
  margin-top: 4px;
}
.empty {
  text-align: center;
  padding: 30px 0;
}
.info-card .info-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin-top: 12px;
}
.info-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.info-value {
  font-size: 13px;
  font-weight: 500;
  word-break: break-all;
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

