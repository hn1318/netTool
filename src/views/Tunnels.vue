<script setup lang="ts">
import { ref, computed } from "vue";
import PasswordField from "../components/PasswordField.vue";
import Spinner from "../components/Spinner.vue";
import EmptyState from "../components/EmptyState.vue";
import { toast } from "../composables/toast";
import { parseTomlTunnels, tunnelsToToml, genId, tunnelAccessUrl } from "../api";
import type { Profile, TunnelConfig } from "../types";

const props = defineProps<{ profile: Profile }>();
const emit = defineEmits<{
  (e: "add", t: TunnelConfig): void;
  (e: "update-tunnel", t: TunnelConfig): void;
  (e: "remove", id: string): void;
  (e: "remove-many", ids: string[]): void;
  (e: "set-enabled-many", payload: { ids: string[]; enabled: boolean }): void;
  (e: "toggle", id: string): void;
}>();

const tunnels = computed(() => props.profile.tunnels);

// ============ 批量管理 ============
const batchMode = ref(false);
const selected = ref<string[]>([]);
const allSelected = computed(
  () => tunnels.value.length > 0 && selected.value.length === tunnels.value.length
);
const someSelected = computed(
  () => selected.value.length > 0 && selected.value.length < tunnels.value.length
);

function toggleBatchMode() {
  batchMode.value = !batchMode.value;
  selected.value = [];
}

function toggleSelect(id: string) {
  const i = selected.value.indexOf(id);
  if (i >= 0) selected.value.splice(i, 1);
  else selected.value.push(id);
}

function toggleSelectAll() {
  selected.value = allSelected.value ? [] : tunnels.value.map((t) => t.id);
}

/** 批量启用 / 禁用 */
function batchSetEnabled(enabled: boolean) {
  if (!selected.value.length) return;
  emit("set-enabled-many", { ids: [...selected.value], enabled });
  toast.success(`已${enabled ? "启用" : "禁用"} ${selected.value.length} 条隧道`);
  selected.value = [];
}

/** 批量删除 */
function batchDelete() {
  if (!selected.value.length) return;
  if (!window.confirm(`确定删除选中的 ${selected.value.length} 条隧道？该操作不可撤销。`)) return;
  const count = selected.value.length;
  emit("remove-many", [...selected.value]);
  toast.success(`已删除 ${count} 条隧道`);
  selected.value = [];
  if (tunnels.value.length === 0) batchMode.value = false;
}

const showImport = ref(false);
const importText = ref("");
const importType = ref<"toml" | "json">("toml");
const importError = ref("");
const importParsing = ref(false);
const copyingId = ref("");

const showEdit = ref(false);
const editingId = ref<string | null>(null);
const editTunnel = ref<Omit<TunnelConfig, "id">>(emptyTunnel());

function emptyTunnel(): Omit<TunnelConfig, "id"> {
  return {
    name: "",
    type: "tcp",
    localIp: "127.0.0.1",
    localPort: 0,
    remotePort: 0,
    customDomains: "",
    subdomain: "",
    secretKey: "",
    useEncryption: true,
    useCompression: false,
    bandwidthLimit: "",
    enabled: true,
    source: "manual",
  };
}

const typeColors: Record<string, string> = {
  tcp: "rgba(79,140,255,.14)",
  udp: "rgba(251,191,36,.14)",
  http: "rgba(52,211,153,.14)",
  https: "rgba(167,139,250,.14)",
  stcp: "rgba(248,113,113,.14)",
  xtcp: "rgba(96,165,250,.14)",
  tcpmux: "rgba(96,165,250,.14)",
};
const typeTextColors: Record<string, string> = {
  tcp: "#4f8cff",
  udp: "#fbbf24",
  http: "#34d399",
  https: "#a78bfa",
  stcp: "#f87171",
  xtcp: "#60a5fa",
  tcpmux: "#60a5fa",
};

const isPortType = computed(() => ["tcp", "udp"].includes(editTunnel.value.type));
const isDomainType = computed(() => ["http", "https", "tcpmux"].includes(editTunnel.value.type));
const isSecretType = computed(() => ["stcp", "xtcp"].includes(editTunnel.value.type));

/** 编辑中的 HTTP(S) 隧道访问地址预览 */
const editAccessUrl = computed(() => {
  const t = editTunnel.value;
  if (t.type !== "http" && t.type !== "https") return "";
  const domain =
    (t.customDomains || "")
      .split(",")
      .map((d) => d.trim())
      .filter(Boolean)[0] || (t.subdomain || "").trim();
  return domain ? `${t.type}://${domain}` : "";
});

/** HTTP(S) 隧道缺少域名时的提示 */
const domainWarning = computed(
  () => isDomainType.value && !editTunnel.value.customDomains?.trim() && !editTunnel.value.subdomain?.trim()
);

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* 忽略 */
  }
}

function parseJsonTunnels(text: string): TunnelConfig[] {
  const data = JSON.parse(text);
  const result: TunnelConfig[] = [];
  const proxies = Array.isArray(data) ? data : data.proxies || [];
  for (const p of proxies) {
    result.push({
      id: genId(),
      name: p.name || "proxy-" + genId(),
      type: p.type || "tcp",
      localIp: p.localIP || p.localIp || "127.0.0.1",
      localPort: Number(p.localPort || 0),
      remotePort: Number(p.remotePort || 0),
      customDomains: Array.isArray(p.customDomains) ? p.customDomains.join(",") : p.customDomains,
      subdomain: p.subdomain,
      secretKey: p.secretKey,
      useEncryption: p.transport?.useEncryption ?? p.useEncryption,
      useCompression: p.transport?.useCompression ?? p.useCompression,
      bandwidthLimit: p.bandwidthLimit,
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
      const result = await parseTomlTunnels(importText.value);
      parsed = result.tunnels;
      if (parsed.length === 0) {
        importError.value = "未在 TOML 中检测到 [[proxies]] 隧道段落";
        return;
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

function openAdd() {
  editingId.value = null;
  editTunnel.value = emptyTunnel();
  showEdit.value = true;
}

function openEdit(t: TunnelConfig) {
  editingId.value = t.id;
  editTunnel.value = {
    name: t.name,
    type: t.type,
    localIp: t.localIp,
    localPort: t.localPort,
    remotePort: t.remotePort,
    customDomains: t.customDomains || "",
    subdomain: t.subdomain || "",
    secretKey: t.secretKey || "",
    useEncryption: t.useEncryption ?? true,
    useCompression: t.useCompression ?? false,
    bandwidthLimit: t.bandwidthLimit || "",
    enabled: t.enabled,
    source: t.source || "manual",
  };
  showEdit.value = true;
}

function saveTunnel() {
  if (!editTunnel.value.name.trim()) return;
  if (editingId.value) {
    emit("update-tunnel", { ...editTunnel.value, id: editingId.value });
  } else {
    emit("add", { ...editTunnel.value, id: genId() });
  }
  showEdit.value = false;
}

async function copyToml(t: TunnelConfig) {
  copyingId.value = t.id;
  try {
    const toml = await tunnelsToToml({
      serverAddr: props.profile.serverAddr || "frp.example.com",
      serverPort: props.profile.serverPort || 7000,
      user: props.profile.user || "",
      authToken: props.profile.authToken || "",
      tunnels: [{ ...t, enabled: true }],
    });
    await navigator.clipboard.writeText(toml);
    toast.success("已复制隧道 TOML");
    window.setTimeout(() => (copyingId.value = ""), 900);
  } catch (e) {
    console.error("复制 TOML 失败", e);
    copyingId.value = "";
  }
}

const enabledCount = computed(() => tunnels.value.filter((t) => t.enabled).length);
</script>

<template>
  <div class="tunnels">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>隧道管理</h2>
        <p class="muted small">
          当前配置：<strong>{{ profile.name }}</strong> · 支持 TOML / INI / JSON 配置导入
        </p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="showImport = true">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
          </svg>
          导入配置
        </button>
        <button class="btn btn-primary" @click="openAdd">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M12 5v14M5 12h14" />
          </svg>
          新建隧道
        </button>
        <button
          class="btn"
          :class="batchMode ? 'btn-ghost' : 'btn-ghost'"
          :disabled="tunnels.length === 0"
          @click="toggleBatchMode"
        >
          {{ batchMode ? "退出批量" : "批量管理" }}
        </button>
      </div>
    </div>

    <div class="summary-row">
      <span class="muted small">共 {{ tunnels.length }} 个隧道 · {{ enabledCount }} 个已启用</span>
      <span class="muted small">· 修改后需在控制面板重新连接才会生效</span>
    </div>

    <!-- 批量操作栏 -->
    <transition name="fade">
      <div v-if="batchMode" class="batch-bar card">
        <label class="batch-check">
          <input
            type="checkbox"
            :checked="allSelected"
            :indeterminate="someSelected"
            @change="toggleSelectAll"
          />
          全选
        </label>
        <span class="muted small">已选 {{ selected.length }} / {{ tunnels.length }}</span>
        <div class="flex gap-2 batch-actions">
          <button class="btn btn-success btn-sm" :disabled="!selected.length" @click="batchSetEnabled(true)">
            批量启用
          </button>
          <button class="btn btn-ghost btn-sm" :disabled="!selected.length" @click="batchSetEnabled(false)">
            批量禁用
          </button>
          <button class="btn btn-danger btn-sm" :disabled="!selected.length" @click="batchDelete">
            批量删除
          </button>
        </div>
      </div>
    </transition>

    <div class="tunnel-list">
      <transition-group name="list">
        <div
          v-for="t in tunnels"
          :key="t.id"
          class="card tunnel-item"
          :class="{ selected: batchMode && selected.includes(t.id) }"
        >
          <input
            v-if="batchMode"
            class="batch-item-check"
            type="checkbox"
            :checked="selected.includes(t.id)"
            @change="toggleSelect(t.id)"
          />
          <div class="tunnel-type" :style="{ background: typeColors[t.type], color: typeTextColors[t.type] }">
            {{ t.type.toUpperCase() }}
          </div>
          <div class="tunnel-info">
            <div class="tunnel-name">
              {{ t.name }}
              <span v-if="t.customDomains" class="muted small">· {{ t.customDomains }}</span>
              <span v-if="t.subdomain" class="muted small">· {{ t.subdomain }}</span>
            </div>
            <div class="tunnel-meta muted small">
              {{ t.localIp }}:{{ t.localPort }}
              <span class="arrow">→</span>
              <template v-if="t.type === 'tcp' || t.type === 'udp'">远程端口 {{ t.remotePort }}</template>
              <template v-else-if="t.type === 'stcp' || t.type === 'xtcp'">密钥隧道</template>
              <template v-else>
                <span :class="{ 'warn-text': !tunnelAccessUrl(t) }">
                  {{ tunnelAccessUrl(t) || "未配置访问域名" }}
                </span>
              </template>
            </div>
          </div>
          <span class="badge source-badge">{{ t.source }}</span>
          <button class="icon-btn" title="编辑" @click="openEdit(t)">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z" />
            </svg>
          </button>
          <button class="icon-btn" :title="copyingId === t.id ? '已复制' : '复制 TOML'" @click="copyToml(t)">
            <svg v-if="copyingId !== t.id" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" />
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="m20 6-11 11-5-5" />
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
        <EmptyState
          title="还没有隧道"
          description="新建一条隧道，或粘贴 frpc.toml / 旧版 INI / JSON 快速导入。保存后在控制面板重新连接即可生效。"
        >
          <template #action>
            <div class="flex gap-2">
              <button class="btn btn-primary" @click="openAdd">新建隧道</button>
              <button class="btn btn-ghost" @click="showImport = true">导入配置</button>
            </div>
          </template>
        </EmptyState>
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
              :placeholder="importType === 'toml' ? '粘贴 frpc.toml（含 [[proxies]]）或旧版 INI（[common] + [隧道名]）...' : '粘贴 JSON 配置...'"
            ></textarea>
            <p v-if="importError" class="error-text">{{ importError }}</p>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="showImport = false">取消</button>
            <button class="btn btn-primary" :disabled="importParsing" @click="doImport">
              <Spinner v-if="importParsing" :size="13" :width="2" />
              {{ importParsing ? "解析中..." : "导入" }}
            </button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 新建/编辑弹窗 -->
    <transition name="modal">
      <div v-if="showEdit" class="modal-mask" @click.self="showEdit = false">
        <div class="modal">
          <div class="modal-header">
            <h3>{{ editingId ? "编辑隧道" : "新建隧道" }}</h3>
            <button class="icon-btn" @click="showEdit = false">✕</button>
          </div>
          <div class="modal-body">
            <div class="form-grid">
              <div class="field">
                <label>隧道名称</label>
                <input class="input" v-model="editTunnel.name" placeholder="例如 ssh" />
              </div>
              <div class="field">
                <label>类型</label>
                <select class="select" v-model="editTunnel.type">
                  <option value="tcp">TCP</option>
                  <option value="udp">UDP</option>
                  <option value="http">HTTP</option>
                  <option value="https">HTTPS</option>
                  <option value="stcp">STCP</option>
                  <option value="xtcp">XTCP</option>
                  <option value="tcpmux">TCPMUX</option>
                </select>
              </div>
              <div class="field">
                <label>本地 IP</label>
                <input class="input" v-model="editTunnel.localIp" />
              </div>
              <div class="field">
                <label>本地端口（要穿透的服务端口）</label>
                <input class="input" type="number" v-model.number="editTunnel.localPort" />
              </div>
              <div v-if="isPortType" class="field">
                <label>远程端口（服务端暴露端口）</label>
                <input class="input" type="number" v-model.number="editTunnel.remotePort" />
              </div>
              <div v-if="isDomainType" class="field">
                <label>自定义域名（逗号分隔）</label>
                <input class="input" v-model="editTunnel.customDomains" placeholder="web.example.com" />
              </div>
              <div v-if="isDomainType" class="field">
                <label>子域名（subdomain）</label>
                <input class="input" v-model="editTunnel.subdomain" placeholder="可选" />
              </div>
              <div v-if="isSecretType" class="field">
                <label>密钥（secretKey）</label>
                <PasswordField v-model="editTunnel.secretKey" placeholder="访问方需使用同一密钥" />
              </div>
              <div class="field">
                <label>带宽限制（bandwidthLimit）</label>
                <input class="input" v-model="editTunnel.bandwidthLimit" placeholder="可选，如 1MB" />
              </div>
            </div>

            <div class="transport-section">
              <h4 class="transport-title">传输选项 (transport)</h4>
              <div class="flex gap-4">
                <label class="checkbox-field">
                  <input type="checkbox" v-model="editTunnel.useEncryption" />
                  启用加密 (useEncryption)
                </label>
                <label class="checkbox-field">
                  <input type="checkbox" v-model="editTunnel.useCompression" />
                  启用压缩 (useCompression)
                </label>
              </div>
            </div>

            <div v-if="isDomainType" class="access-section">
              <h4 class="transport-title">访问地址</h4>
              <div v-if="editAccessUrl" class="access-row">
                <code class="access-url">{{ editAccessUrl }}</code>
                <button class="btn btn-ghost btn-xs" @click="copyText(editAccessUrl)">复制</button>
              </div>
              <p v-else class="warn-text small">
                HTTP / HTTPS 隧道需要填写自定义域名或子域名，否则 frpc 无法建立 vhost 隧道
              </p>
              <p class="muted small">
                域名需在 frps 服务端已绑定（`vhostHTTPPort` / `vhostHTTPSPort`），并由 DNS 解析到服务端
              </p>
              <p v-if="domainWarning" class="muted small">当前仍可保存，但启动前必须补全域名</p>
            </div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="showEdit = false">取消</button>
            <button class="btn btn-primary" @click="saveTunnel">{{ editingId ? "保存" : "创建" }}</button>
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
  display: flex;
  gap: 8px;
  margin: 12px 0;
  flex-wrap: wrap;
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
.tunnel-item.selected {
  border-color: var(--primary);
  background: var(--primary-soft);
}
.batch-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 16px;
  flex-wrap: wrap;
}
.batch-actions {
  margin-left: auto;
}
.batch-check {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.batch-check input,
.batch-item-check {
  width: 15px;
  height: 15px;
  accent-color: var(--primary);
  cursor: pointer;
}
.batch-item-check {
  flex-shrink: 0;
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
  min-width: 0;
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
  width: 640px;
  max-width: 92vw;
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
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 14px;
}
.transport-section {
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}
.transport-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 12px;
  color: var(--text-secondary);
}
.checkbox-field {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text);
  cursor: pointer;
}
.checkbox-field input {
  width: 16px;
  height: 16px;
  accent-color: var(--primary);
}
.access-section {
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.access-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.access-url {
  flex: 1;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
  color: var(--primary);
  word-break: break-all;
}
.warn-text {
  color: var(--warning);
}
.btn-xs {
  padding: 3px 8px;
  font-size: 11px;
}
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
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
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

