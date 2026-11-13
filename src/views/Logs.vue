<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from "vue";
import EmptyState from "../components/EmptyState.vue";
import { loadStore, saveStore } from "../api";
import type { LogEntry, LogLevel } from "../types";

const props = defineProps<{ logs: LogEntry[]; frpRunning?: boolean }>();
const emit = defineEmits<{
  (e: "clear"): void;
}>();

const filter = ref<LogLevel | "all">(loadStore<LogLevel | "all">("log_filter", "all"));
const search = ref("");
const autoScroll = ref(loadStore<boolean>("log_autoscroll", true));
const logView = ref<HTMLElement | null>(null);

const levels: (LogLevel | "all")[] = ["all", "trace", "debug", "info", "warn", "error"];

const levelColors: Record<LogLevel, string> = {
  trace: "#6b7280",
  debug: "#60a5fa",
  info: "#34d399",
  warn: "#fbbf24",
  error: "#f87171",
};
const levelBg: Record<LogLevel, string> = {
  trace: "rgba(107,114,128,.15)",
  debug: "rgba(96,165,250,.15)",
  info: "rgba(52,211,153,.15)",
  warn: "rgba(251,191,36,.15)",
  error: "rgba(248,113,113,.15)",
};

function setFilter(l: LogLevel | "all") {
  filter.value = l;
  saveStore("log_filter", l);
}

function scrollToBottom() {
  const el = logView.value;
  if (el) el.scrollTop = el.scrollHeight;
}

function toggleAutoScroll() {
  autoScroll.value = !autoScroll.value;
  saveStore("log_autoscroll", autoScroll.value);
  if (autoScroll.value) scrollToBottom();
}

// 有新日志 / 切换筛选时，若开启自动滚动则贴底
watch(
  () => [props.logs.length, filter.value, search.value],
  async () => {
    if (!autoScroll.value) return;
    await nextTick();
    scrollToBottom();
  }
);

onMounted(() => {
  if (autoScroll.value) scrollToBottom();
});

const filteredLogs = computed(() => {
  let list = props.logs;
  if (filter.value !== "all") {
    list = list.filter((l) => l.level === filter.value);
  }
  if (search.value.trim()) {
    const q = search.value.toLowerCase();
    list = list.filter((l) => l.message.toLowerCase().includes(q));
  }
  return list;
});

const counts = computed(() => {
  const c: Record<string, number> = { all: props.logs.length };
  for (const l of props.logs) {
    c[l.level] = (c[l.level] || 0) + 1;
  }
  return c;
});

function exportLogs() {
  const text = filteredLogs.value
    .map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] ${l.message}`)
    .join("\n");
  const blob = new Blob([text], { type: "text/plain" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `frpc-logs-${Date.now()}.log`;
  a.click();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <div class="logs">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>日志</h2>
        <p class="muted small">frpc 运行的实时日志</p>
      </div>
      <div class="flex gap-2 items-center">
        <div class="live-badge" :class="{ active: frpRunning }">
          <span class="status-dot" :class="{ on: frpRunning }"></span>
          {{ frpRunning ? "frp 运行中" : "未运行" }}
        </div>
        <button class="btn btn-ghost" @click="emit('clear')">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
          </svg>
          清空
        </button>
        <button class="btn btn-primary" @click="exportLogs">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
          </svg>
          导出
        </button>
      </div>
    </div>

    <div class="toolbar card">
      <div class="filters">
        <button
          v-for="l in levels"
          :key="l"
          class="filter-btn"
          :class="{ active: filter === l }"
          :style="l !== 'all' ? { '--lv-color': levelColors[l] } : {}"
          @click="setFilter(l)"
        >
          {{ l.toUpperCase() }}
          <span class="count">{{ counts[l] || 0 }}</span>
        </button>
      </div>
      <div class="flex items-center gap-2" style="flex: 1">
        <input class="input search" v-model="search" placeholder="搜索日志..." />
        <label class="autoscroll">
          <input type="checkbox" :checked="autoScroll" @change="toggleAutoScroll" />
          自动滚动
        </label>
      </div>
    </div>

    <div class="card log-view" ref="logView">
      <transition-group name="log">
        <div v-for="l in filteredLogs" :key="l.id" class="log-line">
          <span class="log-time muted">{{ l.timestamp }}</span>
          <span class="log-level" :style="{ color: levelColors[l.level], background: levelBg[l.level] }">
            {{ l.level.toUpperCase() }}
          </span>
          <span class="log-msg">{{ l.message }}</span>
        </div>
      </transition-group>
      <div v-if="filteredLogs.length === 0" class="empty">
        <EmptyState
          compact
          :title="props.logs.length ? '没有匹配的日志' : '暂无日志'"
          :description="
            props.logs.length
              ? '试试切换级别或清空搜索关键词'
              : '连接隧道后，frpc 的运行日志会实时出现在这里'
          "
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.logs {
  display: flex;
  flex-direction: column;
  height: 100%;
  animation: fadeUp 0.4s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  margin: 16px 0;
  flex-wrap: wrap;
}
.filters {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.live-badge {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 5px 12px;
  border-radius: 999px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.live-badge.active {
  color: var(--success);
  border-color: rgba(52, 211, 153, 0.3);
  background: rgba(52, 211, 153, 0.1);
}
.filter-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 999px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  transition: all var(--transition);
}
.filter-btn:hover {
  border-color: var(--border-light);
  color: var(--text);
}
.filter-btn.active {
  border-color: var(--lv-color, var(--primary));
  color: var(--lv-color, var(--primary));
  background: var(--primary-soft);
}
.filter-btn .count {
  font-size: 10px;
  background: var(--bg-hover);
  padding: 1px 6px;
  border-radius: 999px;
}
.search {
  max-width: 260px;
}
.autoscroll {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  white-space: nowrap;
}
.log-view {
  flex: 1;
  overflow-y: auto;
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
  padding: 14px;
  min-height: 200px;
}
.log-line {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 5px 8px;
  border-radius: 4px;
  transition: background var(--transition);
}
.log-line:hover {
  background: var(--bg-hover);
}
.log-time {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-muted);
}
.log-level {
  flex-shrink: 0;
  width: 58px;
  text-align: center;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 4px;
  border-radius: 4px;
}
.log-msg {
  word-break: break-all;
  color: var(--text);
}
.empty {
  text-align: center;
  padding: 40px 0;
}
.log-enter-active {
  transition: all 0.3s ease;
}
.log-enter-from {
  opacity: 0;
  transform: translateX(-10px);
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
