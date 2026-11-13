<script setup lang="ts">
import { ref, computed } from "vue";
import PasswordField from "../components/PasswordField.vue";
import Spinner from "../components/Spinner.vue";
import { toast } from "../composables/toast";
import { parseTomlTunnels, previewConfig } from "../api";
import type { Profile } from "../types";

const props = defineProps<{ profiles: Profile[]; activeId: string }>();
const emit = defineEmits<{
  (e: "create-profile", preset?: Partial<Profile>): void;
  (e: "replace-profile", p: Profile): void;
  (e: "delete-profile", id: string): void;
  (e: "delete-profiles", ids: string[]): void;
  (e: "duplicate-profile", id: string): void;
  (e: "activate-profile", id: string): void;
  (e: "update-profile", patch: Partial<Profile>): void;
}>();

type Draft = {
  id: string | null;
  name: string;
  serverAddr: string;
  serverPort: number;
  user: string;
  authToken: string;
  transport: Profile["transport"];
  tlsEnable: boolean;
  logLevel: Profile["logLevel"];
};

const showEdit = ref(false);
const draft = ref<Draft>(emptyDraft());
const showImport = ref(false);
const importText = ref("");
const importError = ref("");
const importParsing = ref(false);
const exportingId = ref("");

// ============ 批量管理 ============
const batchMode = ref(false);
const selected = ref<string[]>([]);
const allSelected = computed(
  () => props.profiles.length > 0 && selected.value.length === props.profiles.length
);
const someSelected = computed(
  () => selected.value.length > 0 && selected.value.length < props.profiles.length
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
  selected.value = allSelected.value ? [] : props.profiles.map((p) => p.id);
}

/** 批量删除（至少保留一份配置由 App 侧兜底） */
function batchDelete() {
  if (!selected.value.length) return;
  if (!window.confirm(`确定删除选中的 ${selected.value.length} 套配置？其隧道会一并删除。`)) return;
  emit("delete-profiles", [...selected.value]);
  toast.success(`已删除 ${selected.value.length} 套配置`);
  selected.value = [];
  batchMode.value = false;
}

/** 批量导出：逐套生成 frpc.toml 并下载 */
async function batchExport() {
  const targets = props.profiles.filter((p) => selected.value.includes(p.id));
  for (const p of targets) {
    await exportToml(p);
    // 浏览器对连续下载有节流，稍作间隔
    await new Promise((r) => window.setTimeout(r, 300));
  }
  if (targets.length) toast.success(`已导出 ${targets.length} 套配置`);
}

function emptyDraft(): Draft {
  return {
    id: null,
    name: "",
    serverAddr: "",
    serverPort: 7000,
    user: "",
    authToken: "",
    transport: "tcp",
    tlsEnable: true,
    logLevel: "info",
  };
}

function openCreate() {
  draft.value = emptyDraft();
  showEdit.value = true;
}

function openEdit(p: Profile) {
  draft.value = {
    id: p.id,
    name: p.name,
    serverAddr: p.serverAddr,
    serverPort: p.serverPort,
    user: p.user,
    authToken: p.authToken,
    transport: p.transport,
    tlsEnable: p.tlsEnable,
    logLevel: p.logLevel,
  };
  showEdit.value = true;
}

function save() {
  const d = draft.value;
  if (!d.name.trim()) return;
  if (d.id) {
    const src = props.profiles.find((p) => p.id === d.id);
    if (src) {
      emit("replace-profile", {
        ...src,
        name: d.name.trim(),
        serverAddr: d.serverAddr.trim(),
        serverPort: Number(d.serverPort) || 7000,
        user: d.user.trim(),
        authToken: d.authToken,
        transport: d.transport,
        tlsEnable: d.tlsEnable,
        logLevel: d.logLevel,
      });
    }
  } else {
    emit("create-profile", {
      name: d.name.trim(),
      serverAddr: d.serverAddr.trim(),
      serverPort: Number(d.serverPort) || 7000,
      user: d.user.trim(),
      authToken: d.authToken,
      transport: d.transport,
      tlsEnable: d.tlsEnable,
      logLevel: d.logLevel,
      tunnels: [],
    });
  }
  showEdit.value = false;
}

async function doImport() {
  importError.value = "";
  if (!importText.value.trim()) {
    importError.value = "请粘贴 frpc.toml 内容";
    return;
  }
  importParsing.value = true;
  try {
    const parsed = await parseTomlTunnels(importText.value);
    if (!parsed.tunnels.length && !parsed.serverAddr) {
      importError.value = "未解析到服务器或隧道信息";
      return;
    }
    emit("create-profile", {
      name: parsed.serverAddr ? `${parsed.serverAddr} 导入` : "导入的配置",
      serverAddr: parsed.serverAddr || "",
      serverPort: parsed.serverPort || 7000,
      user: parsed.user || "",
      authToken: parsed.authToken || "",
      tunnels: parsed.tunnels,
    });
    showImport.value = false;
    importText.value = "";
  } catch (e) {
    importError.value = "解析失败：" + String(e);
  } finally {
    importParsing.value = false;
  }
}

async function exportToml(p: Profile) {
  exportingId.value = p.id;
  try {
    const toml = await previewConfig(p);
    const blob = new Blob([toml], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${p.name || "frpc"}.toml`;
    a.click();
    URL.revokeObjectURL(url);
  } catch (e) {
    importError.value = String(e);
  } finally {
    exportingId.value = "";
  }
}

function confirmDelete(p: Profile) {
  if (props.profiles.length <= 1) {
    alert("至少需要保留一份配置");
    return;
  }
  if (window.confirm(`确定删除配置「${p.name}」？该配置下的隧道将一并删除。`)) {
    emit("delete-profile", p.id);
  }
}

function fmtTime(ts: number) {
  if (!ts) return "-";
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const runningHint = computed(() => "切换配置仅改变当前编辑对象，已连接的隧道需重新连接才会使用新配置");

const PROVIDER_NAMES: Record<string, string> = {
  mefrp: "ME Frp",
  openfrp: "OpenFrp",
  chmlfrp: "ChmlFrp",
  sakura: "SakuraFrp",
  msl: "MSL Frp",
};

function providerName(id?: string) {
  if (!id) return "";
  return PROVIDER_NAMES[id] || id;
}
</script>

<template>
  <div class="profiles">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>配置管理</h2>
        <p class="muted small">每套配置包含独立的服务器参数与隧道列表，可快速切换</p>
      </div>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="showImport = true">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3v12" /><path d="m8 7 4-4 4 4" /><path d="M4 21h16" />
          </svg>
          导入 TOML
        </button>
        <button class="btn btn-primary" @click="openCreate">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M12 5v14M5 12h14" />
          </svg>
          新建配置
        </button>
        <button
          class="btn btn-ghost"
          :disabled="profiles.length === 0"
          @click="toggleBatchMode"
        >
          {{ batchMode ? "退出批量" : "批量管理" }}
        </button>
      </div>
    </div>

    <p class="hint muted small">{{ runningHint }}</p>

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
        <span class="muted small">已选 {{ selected.length }} / {{ profiles.length }}</span>
        <div class="flex gap-2 batch-actions">
          <button
            class="btn btn-ghost btn-sm"
            :disabled="!selected.length || !!exportingId"
            @click="batchExport"
          >
            <Spinner v-if="!!exportingId" :size="11" :width="2" />
            批量导出
          </button>
          <button class="btn btn-danger btn-sm" :disabled="!selected.length" @click="batchDelete">
            批量删除
          </button>
        </div>
      </div>
    </transition>

    <div class="profile-grid">
      <div
        v-for="p in profiles"
        :key="p.id"
        class="card profile-card"
        :class="{ active: p.id === activeId, selected: batchMode && selected.includes(p.id) }"
      >
        <input
          v-if="batchMode"
          class="batch-item-check"
          type="checkbox"
          :checked="selected.includes(p.id)"
          @change="toggleSelect(p.id)"
        />
        <div class="profile-top">
          <div class="profile-name">
            {{ p.name }}
            <span v-if="p.id === activeId" class="badge badge-active">当前</span>
            <span v-if="p.provider" class="badge provider-badge">
              {{ providerName(p.provider) }}
            </span>
          </div>
          <span class="badge transport-badge">{{ p.transport.toUpperCase() }}</span>
        </div>

        <div class="profile-server mono">
          {{ p.serverAddr || "未设置服务器" }}<span v-if="p.serverAddr">:{{ p.serverPort }}</span>
        </div>

        <div class="profile-meta">
          <span>{{ p.tunnels.length }} 条隧道</span>
          <span>·</span>
          <span>{{ p.tunnels.filter((t) => t.enabled).length }} 条启用</span>
          <span>·</span>
          <span>{{ p.tlsEnable ? "TLS 开" : "TLS 关" }}</span>
        </div>
        <div class="muted small">更新于 {{ fmtTime(p.updatedAt) }}</div>

        <div class="profile-actions">
          <button
            class="btn btn-ghost btn-sm"
            :disabled="p.id === activeId"
            @click="emit('activate-profile', p.id)"
          >
            设为当前
          </button>
          <button class="btn btn-ghost btn-sm" @click="openEdit(p)">编辑</button>
          <button class="btn btn-ghost btn-sm" @click="emit('duplicate-profile', p.id)">复制</button>
          <button class="btn btn-ghost btn-sm" :disabled="exportingId === p.id" @click="exportToml(p)">
            <Spinner v-if="exportingId === p.id" :size="11" :width="2" />
            {{ exportingId === p.id ? "导出中" : "导出" }}
          </button>
          <button class="btn btn-danger btn-sm" @click="confirmDelete(p)">删除</button>
        </div>
      </div>
    </div>

    <!-- 新建 / 编辑 -->
    <transition name="modal">
      <div v-if="showEdit" class="modal-mask" @click.self="showEdit = false">
        <div class="modal">
          <div class="modal-header">
            <h3>{{ draft.id ? "编辑配置" : "新建配置" }}</h3>
            <button class="icon-btn" @click="showEdit = false">✕</button>
          </div>
          <div class="modal-body">
            <div class="form-grid">
              <div class="field">
                <label>配置名称</label>
                <input class="input" v-model="draft.name" placeholder="例如 公司服务器" />
              </div>
              <div class="field">
                <label>服务器地址 (serverAddr)</label>
                <input class="input" v-model="draft.serverAddr" placeholder="frp.example.com" />
              </div>
              <div class="field">
                <label>服务器端口 (serverPort)</label>
                <input class="input" type="number" v-model.number="draft.serverPort" />
              </div>
              <div class="field">
                <label>用户 (user)</label>
                <input class="input" v-model="draft.user" placeholder="可选" />
              </div>
              <div class="field">
                <label>认证令牌 (auth.token)</label>
                <PasswordField v-model="draft.authToken" placeholder="可选" />
              </div>
              <div class="field">
                <label>传输协议 (transport.protocol)</label>
                <select class="select" v-model="draft.transport">
                  <option value="tcp">tcp</option>
                  <option value="kcp">kcp</option>
                  <option value="websocket">websocket</option>
                </select>
              </div>
              <div class="field">
                <label>日志级别 (log.level)</label>
                <select class="select" v-model="draft.logLevel">
                  <option value="debug">debug</option>
                  <option value="info">info</option>
                  <option value="warn">warn</option>
                  <option value="error">error</option>
                </select>
              </div>
              <div class="field">
                <label>TLS 加密</label>
                <label class="switch">
                  <input type="checkbox" v-model="draft.tlsEnable" />
                  <span class="slider"></span>
                </label>
              </div>
            </div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="showEdit = false">取消</button>
            <button class="btn btn-primary" @click="save">保存</button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 导入 -->
    <transition name="modal">
      <div v-if="showImport" class="modal-mask" @click.self="showImport = false">
        <div class="modal">
          <div class="modal-header">
            <h3>导入 frpc.toml</h3>
            <button class="icon-btn" @click="showImport = false">✕</button>
          </div>
          <div class="modal-body">
            <textarea
              v-model="importText"
              class="input code-area"
              rows="14"
              placeholder="粘贴完整 frpc.toml（含 serverAddr / serverPort / [[proxies]]）..."
            ></textarea>
            <p class="muted small mt-2">
              导入后会创建一份新配置，服务器参数与所有隧道会一并读取
            </p>
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
  </div>
</template>

<style scoped>
.profiles {
  animation: fadeUp 0.4s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.hint {
  margin-top: 10px;
}
.profile-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 16px;
  margin-top: 16px;
}
.profile-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-color: var(--border);
  position: relative;
}
.profile-card.active {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary-soft), var(--shadow);
}
.profile-card.selected {
  border-color: var(--primary);
  background: var(--primary-soft);
}
.batch-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 16px;
  margin-top: 14px;
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
  align-self: flex-start;
}
.profile-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.profile-name {
  font-size: 15px;
  font-weight: 600;
  display: flex;
  align-items: center;
  gap: 8px;
}
.badge-active {
  background: var(--primary-soft);
  color: var(--primary);
}
.provider-badge {
  background: rgba(167, 139, 250, 0.14);
  color: #a78bfa;
}
.transport-badge {
  background: var(--bg-hover);
  color: var(--text-muted);
}
.profile-server {
  font-size: 13px;
  color: var(--text-secondary);
  word-break: break-all;
}
.mono {
  font-family: "JetBrains Mono", "Fira Code", monospace;
}
.profile-meta {
  display: flex;
  gap: 6px;
  font-size: 12px;
  color: var(--text-secondary);
  flex-wrap: wrap;
}
.profile-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}
.btn-sm {
  padding: 5px 10px;
  font-size: 12px;
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
  width: 620px;
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
.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 14px;
}
.code-area {
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
  line-height: 1.5;
  resize: vertical;
}
.error-text {
  color: var(--danger);
  font-size: 12px;
  margin-top: 8px;
}
.icon-btn {
  background: transparent;
  color: var(--text-muted);
  padding: 6px;
  border-radius: 6px;
}
.icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.switch {
  position: relative;
  display: inline-block;
  width: 42px;
  height: 24px;
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

