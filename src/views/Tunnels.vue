<script setup lang="ts">
import { ref, computed } from "vue";
import { parseToml, jsonToToml, genId } from "../api";
import type { TunnelConfig } from "../types";

const props = defineProps<{ tunnels: TunnelConfig[] }>();
const emit = defineEmits<{
  (e: "add", t: TunnelConfig): void;
  (e: "remove", id: string): void;
  (e: "toggle", id: string): void;
}>();

const showImport = ref(false);
const importText = ref("");
const importType = ref<"toml" | "json">("toml");
const importError = ref("");
const importParsing = ref(false);

const showAdd = ref(false);
const newTunnel = ref<Omit<TunnelConfig, "id">>({
  name: "",
  type: "tcp",
  localIp: "127.0.0.1",
  localPort: 0,
  remotePort: 0,
  enabled: true,
  source: "manual",
});

const typeColors: Record<string, string> = {
  tcp: "rgba(79,140,255,.14)",
  udp: "rgba(251,191,36,.14)",
  http: "rgba(52,211,153,.14)",
  https: "rgba(167,139,250,.14)",
  stcp: "rgba(248,113,113,.14)",
  xtcp: "rgba(96,165,250,.14)",
};
const typeTextColors: Record<string, string> = {
  tcp: "#4f8cff",
  udp: "#fbbf24",
  http: "#34d399",
  https: "#a78bfa",
  stcp: "#f87171",
  xtcp: "#60a5fa",
};

// 从 TOML 文本解析隧道
function parseTomlTunnels(text: string): TunnelConfig[] {
  // 先尝试用 Rust 端解析（异步），这里做同步的简化解析
  const result: TunnelConfig[] = [];
  // 匹配 [[proxies]] 段落中的 name/type/localIP/localPort/remotePort
  const proxyBlocks = text.split(/\[\[proxies\]\]/).slice(1);
  for (const block of proxyBlocks) {
    const get = (key: string) => {
      const m = block.match(new RegExp(`${key}\\s*=\\s*"([^"]*)"`));
      return m ? m[1] : "";
    };
    const name = get("name") || "proxy-" + genId();
    const type = (get("type") || "tcp") as TunnelConfig["type"];
    const localIp = get("localIP") || "127.0.0.1";
    const localPort = parseInt(get("localPort") || "0");
    const remotePort = parseInt(get("remotePort") || "0");
    const customDomains = get("customDomains");
    result.push({
      id: genId(),
      name,
      type,
      localIp,
      localPort,
      remotePort,
      customDomains,
      enabled: true,
      source: "toml",
    });
  }
  return result;
}

function parseJsonTunnels(text: string): TunnelConfig[] {
  const data = JSON.parse(text);
  const result: TunnelConfig[] = [];
  // 支持 {"proxies": [...]} 或直接数组
  const proxies = Array.isArray(data) ? data : data.proxies || [];
  for (const p of proxies) {
    result.push({
      id: genId(),
      name: p.name || "proxy-" + genId(),
      type: p.type || "tcp",
      localIp: p.localIP || "127.0.0.1",
      localPort: Number(p.localPort || 0),
      remotePort: Number(p.remotePort || 0),
      customDomains: p.customDomains,
      enabled: true,
      source: "json",
    });
  }
  return result;
}

async function doImport() {
  importError.value = "";
  if (!importText.value.trim()) {
    importError.value = "请输入配置内容";
    return;
  }
  importParsing.value = true;
  try {
    let parsed: TunnelConfig[] = [];
    if (importType.value === "toml") {
      parsed = parseTomlTunnels(importText.value);
      // 若同步解析为空，尝试后端解析
      if (parsed.length === 0) {
        try {
          await parseToml(importText.value);
          importError.value = "未在 TOML 中检测到 [[proxies]] 隧道段落";
          return;
        } catch (e) {
          importError.value = "TOML 解析失败：" + String(e);
          return;
        }
      }
    } else {
      parsed = parseJsonTunnels(importText.value);
    }
    if (parsed.length === 0) {
      importError.value = "未解析到任何隧道";
      return;
    }
    for (const t of parsed) emit("add", t);
    showImport.value = false;
    importText.value = "";
  } catch (e) {
    importError.value = "解析失败：" + String(e);
  } finally {
    importParsing.value = false;
  }
}

function addTunnel() {
  if (!newTunnel.value.name) return;
  emit("add", { ...newTunnel.value, id: genId() });
  newTunnel.value = {
    name: "",
    type: "tcp",
    localIp: "127.0.0.1",
    localPort: 0,
    remotePort: 0,
    enabled: true,
    source: "manual",
  };
  showAdd.value = false;
}

async function copyToml(t: TunnelConfig) {
  try {
    const toml = await jsonToToml(JSON.stringify({ proxies: [t] }));
    await navigator.clipboard.writeText(toml);
  } catch {
    /* ignore */
  }
}

const enabledCount = computed(() => props.tunnels.filter((t) => t.enabled).length);
</script>

<template>
  <div class="tunnels">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>隧道管理</h2>
        <p class="muted small">管理 FRP 隧道，支持 TOML / JSON 配置导入</p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="showImport = true">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
          </svg>
          导入配置
        </button>
        <button class="btn btn-primary" @click="showAdd = true">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M12 5v14M5 12h14" />
          </svg>
          新建隧道
        </button>
      </div>
    </div>

    <div class="summary-row">
      <span class="muted small">共 {{ tunnels.length }} 个隧道 · {{ enabledCount }} 个已启用</span>
    </div>

    <!-- 隧道列表 -->
    <div class="tunnel-list">
      <transition-group name="list">
        <div v-for="t in tunnels" :key="t.id" class="card tunnel-item">
          <div class="tunnel-type" :style="{ background: typeColors[t.type], color: typeTextColors[t.type] }">
            {{ t.type.toUpperCase() }}
          </div>
          <div class="tunnel-info">
            <div class="tunnel-name">
              {{ t.name }}
              <span v-if="t.customDomains" class="muted small">· {{ t.customDomains }}</span>
            </div>
            <div class="tunnel-meta muted small">
              {{ t.localIp }}:{{ t.localPort }}
              <span class="arrow">→</span>
              远程端口 {{ t.remotePort }}
            </div>
          </div>
          <span class="badge source-badge">{{ t.source }}</span>
          <button class="icon-btn" title="复制 TOML" @click="copyToml(t)">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
          </button>
          <button class="icon-btn danger" title="删除" @click="emit('remove', t.id)">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
            </svg>
          </button>
          <label class="switch" :title="t.enabled ? '禁用' : '启用'">
            <input type="checkbox" :checked="t.enabled" @change="emit('toggle', t.id)" />
            <span class="slider"></span>
          </label>
        </div>
      </transition-group>
      <div v-if="tunnels.length === 0" class="empty">
        <p class="muted">暂无隧道，点击「新建隧道」或「导入配置」开始</p>
      </div>
    </div>

    <!-- 导入弹窗 -->
    <transition name="modal">
      <div v-if="showImport" class="modal-mask" @click.self="showImport = false">
        <div class="modal">
          <div class="modal-header">
            <h3>导入隧道配置</h3>
            <button class="icon-btn" @click="showImport = false">✕</button>
          </div>
          <div class="modal-body">
            <div class="flex gap-2" style="margin-bottom: 12px">
              <button class="btn" :class="importType === 'toml' ? 'btn-primary' : 'btn-ghost'" @click="importType = 'toml'">
                TOML 配置
              </button>
              <button class="btn" :class="importType === 'json' ? 'btn-primary' : 'btn-ghost'" @click="importType = 'json'">
                JSON 配置
              </button>
            </div>
            <textarea
              v-model="importText"
              class="input code-area"
              rows="12"
              :placeholder="importType === 'toml' ? '粘贴 [[proxies]] 配置...' : '粘贴 JSON 配置...'"
            ></textarea>
            <p v-if="importError" class="error-text">{{ importError }}</p>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="showImport = false">取消</button>
            <button class="btn btn-primary" :disabled="importParsing" @click="doImport">
              {{ importParsing ? "解析中..." : "导入" }}
            </button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 新建弹窗 -->
    <transition name="modal">
      <div v-if="showAdd" class="modal-mask" @click.self="showAdd = false">
        <div class="modal">
          <div class="modal-header">
            <h3>新建隧道</h3>
            <button class="icon-btn" @click="showAdd = false">✕</button>
          </div>
          <div class="modal-body">
            <div class="form-grid">
              <div class="field">
                <label>隧道名称</label>
                <input class="input" v-model="newTunnel.name" placeholder="例如 ssh" />
              </div>
              <div class="field">
                <label>类型</label>
                <select class="select" v-model="newTunnel.type">
                  <option value="tcp">TCP</option>
                  <option value="udp">UDP</option>
                  <option value="http">HTTP</option>
                  <option value="https">HTTPS</option>
                  <option value="stcp">STCP</option>
                  <option value="xtcp">XTCP</option>
                </select>
              </div>
              <div class="field">
                <label>本地 IP</label>
                <input class="input" v-model="newTunnel.localIp" />
              </div>
              <div class="field">
                <label>本地端口</label>
                <input class="input" type="number" v-model.number="newTunnel.localPort" />
              </div>
              <div class="field">
                <label>远程端口</label>
                <input class="input" type="number" v-model.number="newTunnel.remotePort" />
              </div>
              <div class="field">
                <label>自定义域名（HTTP/HTTPS）</label>
                <input class="input" v-model="newTunnel.customDomains" placeholder="可选" />
              </div>
            </div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="showAdd = false">取消</button>
            <button class="btn btn-primary" @click="addTunnel">创建</button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.tunnels {
  animation: fadeUp 0.4s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.summary-row {
  margin: 12px 0;
}
.tunnel-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.tunnel-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px 18px;
}
.tunnel-type {
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.5px;
  flex-shrink: 0;
}
.tunnel-info {
  flex: 1;
}
.tunnel-name {
  font-weight: 600;
  font-size: 14px;
}
.tunnel-meta {
  margin-top: 2px;
}
.tunnel-meta .arrow {
  margin: 0 6px;
  color: var(--primary);
}
.source-badge {
  background: var(--bg-hover);
  color: var(--text-muted);
}
.icon-btn {
  background: transparent;
  color: var(--text-muted);
  padding: 6px;
  border-radius: 6px;
  transition: all var(--transition);
  display: flex;
}
.icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.icon-btn.danger:hover {
  color: var(--danger);
}

/* 开关 */
.switch {
  position: relative;
  display: inline-block;
  width: 42px;
  height: 24px;
  flex-shrink: 0;
  cursor: pointer;
}
.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}
.slider {
  position: absolute;
  inset: 0;
  background: var(--border);
  border-radius: 24px;
  transition: 0.3s;
}
.slider::before {
  content: "";
  position: absolute;
  width: 18px;
  height: 18px;
  left: 3px;
  top: 3px;
  background: #fff;
  border-radius: 50%;
  transition: 0.3s;
}
.switch input:checked + .slider {
  background: var(--primary);
}
.switch input:checked + .slider::before {
  transform: translateX(18px);
}

.empty {
  text-align: center;
  padding: 60px 0;
}

/* 弹窗 */
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  width: 560px;
  max-width: 90vw;
  max-height: 85vh;
  overflow: auto;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 20px;
  border-bottom: 1px solid var(--border);
}
.modal-header h3 {
  font-size: 16px;
}
.modal-body {
  padding: 20px;
}
.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 20px;
  border-top: 1px solid var(--border);
}
.code-area {
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
  resize: vertical;
  line-height: 1.5;
}
.error-text {
  color: var(--danger);
  font-size: 12px;
  margin-top: 8px;
}
.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}

/* 动画 */
.list-enter-active,
.list-leave-active {
  transition: all 0.3s ease;
}
.list-enter-from {
  opacity: 0;
  transform: translateX(-20px);
}
.list-leave-to {
  opacity: 0;
  transform: translateX(20px);
}
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.25s ease;
}
.modal-enter-active .modal,
.modal-leave-active .modal {
  transition: transform 0.25s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from .modal {
  transform: scale(0.95);
}
.modal-leave-to .modal {
  transform: scale(0.95);
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
