<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, markRaw, watch } from "vue";
import Sidebar from "./components/Sidebar.vue";
import PageLoader from "./components/PageLoader.vue";
import EmptyState from "./components/EmptyState.vue";
import ToastHost from "./components/ToastHost.vue";
import { applyAppearance, loadAppearance } from "./appearance";
import Dashboard from "./views/Dashboard.vue";
import Tunnels from "./views/Tunnels.vue";
import Profiles from "./views/Profiles.vue";
import Providers from "./views/Providers.vue";
import CoreDownload from "./views/CoreDownload.vue";
import Settings from "./views/Settings.vue";
import Logs from "./views/Logs.vue";
import UserCenter from "./views/UserCenter.vue";
import About from "./views/About.vue";
import {
  loadStore,
  saveStore,
  loadProfiles,
  saveProfiles,
  newProfileId,
  onFrpLog,
  onFrpStatus,
  getFrpStatus,
  emitLog,
  emptyProfile,
  STORE_KEYS,
  genId,
  closeAction,
  onAppCloseRequest,
  normalizeLogEntries,
} from "./api";
import type { Profile, TunnelConfig, LogEntry, UserInfo } from "./types";

// ============ 导航 ============
const icons = {
  dashboard: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/></svg>`,
  tunnel: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 2a10 10 0 1 0 10 10"/><path d="M12 6a6 6 0 1 0 6 6"/><path d="M12 10a2 2 0 1 0 2 2"/></svg>`,
  profiles: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12 2 9 5-9 5-9-5 9-5z"/><path d="m3 12 9 5 9-5"/><path d="m3 17 9 5 9-5"/></svg>`,
  provider: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 10h-1.26A8 8 0 1 0 9 20h9a4 4 0 0 0 0-8z"/><path d="M12 12v6"/><path d="m9 15 3-3 3 3"/></svg>`,
  download: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`,
  log: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h5"/></svg>`,
  user: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`,
  about: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>`,
};

const navItems = [
  { id: "dashboard", label: "控制面板", icon: icons.dashboard },
  { id: "tunnels", label: "隧道管理", icon: icons.tunnel },
  { id: "profiles", label: "配置管理", icon: icons.profiles },
  { id: "providers", label: "第三方服务", icon: icons.provider },
  { id: "download", label: "核心下载", icon: icons.download },
  { id: "settings", label: "设置", icon: icons.settings },
  { id: "logs", label: "日志", icon: icons.log },
  { id: "user", label: "用户中心", icon: icons.user },
  { id: "about", label: "关于", icon: icons.about },
];

const activeView = ref("dashboard");
const collapsed = ref(false);

// ============ 关闭行为 ============
const showCloseDialog = ref(false);
const rememberClose = ref(false);
let closeBusy = false;

async function resolveClose(action: "tray" | "quit") {
  if (closeBusy) return;
  closeBusy = true;
  showCloseDialog.value = false;
  try {
    await closeAction(action, rememberClose.value);
  } catch (e) {
    console.error("关闭操作失败", e);
  } finally {
    closeBusy = false;
  }
}

function cancelClose() {
  showCloseDialog.value = false;
}

// ============ 主题 ============
const theme = ref(loadStore<string>(STORE_KEYS.theme, "dark"));
function applyTheme(t: string) {
  document.documentElement.setAttribute("data-theme", t);
}
function toggleTheme() {
  theme.value = theme.value === "dark" ? "light" : "dark";
  saveStore(STORE_KEYS.theme, theme.value);
  applyTheme(theme.value);
}
function setTheme(t: string) {
  theme.value = t;
  saveStore(STORE_KEYS.theme, t);
  applyTheme(t);
}

// ============ 配置状态 ============
const profiles = ref<Profile[]>([]);
const activeId = ref("");
const loading = ref(true);
const loadError = ref("");
const saving = ref(false);
const placeholder = emptyProfile("__empty__", "未配置");

const activeProfile = computed<Profile>(
  () => profiles.value.find((p) => p.id === activeId.value) || profiles.value[0] || placeholder
);

let saveTimer: number | undefined;
let savingTimer: number | undefined;
function persist(immediate = false) {
  if (saveTimer) clearTimeout(saveTimer);
  const run = async () => {
    saving.value = true;
    try {
      const saved = await saveProfiles({ activeId: activeId.value, profiles: profiles.value });
      // 后端会补全 id / 时间戳，以返回值为准
      if (saved.profiles.length) {
        profiles.value = saved.profiles;
        activeId.value = saved.activeId;
      }
    } catch (e) {
      console.error("保存配置失败", e);
    } finally {
      if (savingTimer) clearTimeout(savingTimer);
      savingTimer = window.setTimeout(() => (saving.value = false), 350);
    }
  };
  if (immediate) run();
  else saveTimer = window.setTimeout(run, 400);
}

/** 首次运行：把旧的本地存储（单配置）迁移为一份配置 */
function migrateLegacy(): Profile | null {
  const legacyTunnels = loadStore<TunnelConfig[] | null>("tunnels", null);
  const legacySettings = loadStore<any>("settings", null);
  if (!legacyTunnels?.length && !legacySettings) return null;
  const p = emptyProfile(genId(), "默认配置");
  p.serverAddr = legacySettings?.serverAddr || "";
  p.serverPort = legacySettings?.serverPort || 7000;
  p.user = legacySettings?.user || "";
  p.authToken = legacySettings?.authToken || "";
  p.transport = legacySettings?.transport || "tcp";
  p.tlsEnable = legacySettings?.tlsEnable ?? true;
  p.logLevel = legacySettings?.logLevel || "info";
  p.tunnels = (legacyTunnels || []).map((t) => ({ ...t, id: t.id || genId() }));
  return p;
}

/** 首次启动：不再塞入示例隧道，只创建一份空配置，由用户自己填写 */
async function loadAppData() {
  loading.value = true;
  loadError.value = "";
  try {
    const store = await loadProfiles();
    if (store.profiles?.length) {
      profiles.value = store.profiles;
      activeId.value = store.activeId || store.profiles[0].id;
    } else {
      const first = migrateLegacy() || emptyProfile(genId(), "默认配置");
      profiles.value = [first];
      activeId.value = first.id;
      persist(true);
    }
  } catch (e) {
    console.error("加载配置失败", e);
    loadError.value = typeof e === "string" ? e : String(e);
    profiles.value = [];
    activeId.value = "";
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  applyAppearance(loadAppearance());
  applyTheme(theme.value);
  await loadAppData();
  if (!loadError.value) emitLog("info", "NetTool 已启动");
});

// ============ 配置操作 ============
async function createProfile(preset?: Partial<Profile>): Promise<Profile> {
  let id = genId();
  try {
    id = await newProfileId();
  } catch {
    /* 回退本地生成 */
  }
  const p = Object.assign(
    emptyProfile(id, preset?.name?.trim() || `配置 ${profiles.value.length + 1}`),
    preset
  );
  p.id = id;
  p.name = (preset?.name || "").trim() || `配置 ${profiles.value.length + 1}`;
  if (!p.tunnels) p.tunnels = [];
  profiles.value.push(p);
  activeId.value = p.id;
  persist(true);
  return p;
}

function updateProfile(patch: Partial<Profile>) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  Object.assign(target, patch);
  target.updatedAt = Date.now();
  persist();
}

function replaceProfile(profile: Profile) {
  const idx = profiles.value.findIndex((p) => p.id === profile.id);
  if (idx >= 0) {
    profiles.value[idx] = { ...profile, updatedAt: Date.now() };
    persist();
  }
}

function deleteProfile(id: string) {
  profiles.value = profiles.value.filter((p) => p.id !== id);
  if (activeId.value === id) activeId.value = profiles.value[0]?.id || "";
  if (profiles.value.length === 0) {
    createProfile();
  } else {
    persist(true);
  }
}

/** 批量删除配置：至少保留一份，全删时自动新建空配置 */
function deleteProfiles(ids: string[]) {
  const set = new Set(ids);
  if (!set.size) return;
  profiles.value = profiles.value.filter((p) => !set.has(p.id));
  if (!profiles.value.some((p) => p.id === activeId.value)) {
    activeId.value = profiles.value[0]?.id || "";
  }
  if (profiles.value.length === 0) {
    createProfile();
  } else {
    persist(true);
  }
}

function duplicateProfile(id: string) {
  const src = profiles.value.find((p) => p.id === id);
  if (!src) return;
  const copy: Profile = {
    ...JSON.parse(JSON.stringify(src)),
    id: genId(),
    name: `${src.name} 副本`,
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
  profiles.value.push(copy);
  activeId.value = copy.id;
  persist(true);
}

function activateProfile(id: string) {
  activeId.value = id;
  persist(true);
}

/** 重新从磁盘读取配置（第三方服务导入后调用） */
async function reloadProfiles() {
  try {
    const store = await loadProfiles();
    if (store.profiles?.length) {
      profiles.value = store.profiles;
      activeId.value = store.activeId || store.profiles[0].id;
    }
  } catch (e) {
    console.error("重新加载配置失败", e);
  }
}

// ============ 隧道操作（作用于当前配置） ============
function addTunnel(t: TunnelConfig) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  target.tunnels.push(t);
  persist();
}
function updateTunnel(t: TunnelConfig) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  const idx = target.tunnels.findIndex((x) => x.id === t.id);
  if (idx >= 0) target.tunnels[idx] = t;
  persist();
}
function removeTunnel(id: string) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  target.tunnels = target.tunnels.filter((t) => t.id !== id);
  persist();
}
function toggleTunnel(id: string) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  const t = target?.tunnels.find((x) => x.id === id);
  if (t) {
    t.enabled = !t.enabled;
    persist();
  }
}

/** 批量删除隧道 */
function removeTunnels(ids: string[]) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  const set = new Set(ids);
  target.tunnels = target.tunnels.filter((t) => !set.has(t.id));
  persist(true);
}

/** 批量启用 / 禁用隧道 */
function setTunnelsEnabled(payload: { ids: string[]; enabled: boolean }) {
  const target = profiles.value.find((p) => p.id === activeProfile.value.id);
  if (!target) return;
  const set = new Set(payload.ids);
  for (const t of target.tunnels) {
    if (set.has(t.id)) t.enabled = payload.enabled;
  }
  persist(true);
}

// ============ 日志 ============
const logs = ref<LogEntry[]>(
  normalizeLogEntries(loadStore<LogEntry[]>(STORE_KEYS.logs, []))
);
// 清洗后回写一次，避免旧版 ANSI 日志继续污染
saveStore(STORE_KEYS.logs, logs.value);
let logSeq = Date.now();

function timeStr() {
  const d = new Date();
  return `${d.getHours().toString().padStart(2, "0")}:${d
    .getMinutes()
    .toString()
    .padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
}

function addLog(entry: LogEntry) {
  logs.value.push(entry);
  if (logs.value.length > 800) logs.value.splice(0, logs.value.length - 800);
  saveStore(STORE_KEYS.logs, logs.value);
}
function clearLogs() {
  logs.value = [];
  saveStore(STORE_KEYS.logs, []);
}

let unlistenLog: (() => void) | undefined;
let unlistenStatus: (() => void) | undefined;

let unlistenClose: (() => void) | undefined;

/** frpc 是否在运行（日志页用于显示实时状态） */
const frpRunning = ref(false);

onMounted(async () => {
  unlistenClose = await onAppCloseRequest(() => {
    showCloseDialog.value = true;
  });
  unlistenLog = await onFrpLog((data: any) => {
    addLog({
      id: logSeq++,
      timestamp: data.timestamp || timeStr(),
      level: data.level || "info",
      message: data.message || "",
    });
  });
  unlistenStatus = await onFrpStatus((data) => {
    frpRunning.value = data.running;
    if (!data.running && data.pid === 0) {
      addLog({ id: logSeq++, timestamp: timeStr(), level: "info", message: "frpc 已断开" });
    }
  });
  try {
    const status = await getFrpStatus();
    frpRunning.value = !!status?.running;
  } catch {
    /* 忽略 */
  }
});

onBeforeUnmount(() => {
  unlistenLog?.();
  unlistenStatus?.();
  unlistenClose?.();
});

// ============ 用户信息 ============
/** 本地资料默认留空，由用户自己填写；不再预置演示账号 */
const EMPTY_USER: UserInfo = {
  nickname: "",
  email: "",
  avatar: "",
  plan: "",
  expireAt: "",
  bandwidth: "",
};
const storedUser = loadStore<UserInfo>(STORE_KEYS.user, EMPTY_USER);
// 清理旧版本写入的演示资料
const isLegacyMockUser =
  storedUser?.nickname === "开发者" && storedUser?.email === "dev@example.com";
const user = ref<UserInfo>(isLegacyMockUser ? { ...EMPTY_USER } : { ...EMPTY_USER, ...storedUser });
if (isLegacyMockUser) saveStore(STORE_KEYS.user, user.value);

function updateUser(u: UserInfo) {
  user.value = u;
  saveStore(STORE_KEYS.user, u);
}

// 视图映射
const viewMap = {
  dashboard: markRaw(Dashboard),
  tunnels: markRaw(Tunnels),
  profiles: markRaw(Profiles),
  providers: markRaw(Providers),
  download: markRaw(CoreDownload),
  settings: markRaw(Settings),
  logs: markRaw(Logs),
  user: markRaw(UserCenter),
  about: markRaw(About),
};

// 主题变化时通知图表重绘
watch(theme, (t) => applyTheme(t));
</script>

<template>
  <div class="app-shell">
    <Sidebar
      :items="navItems"
      :active="activeView"
      :collapsed="collapsed"
      :theme="theme"
      @select="activeView = $event"
      @toggle="collapsed = !collapsed"
      @toggle-theme="toggleTheme"
    />

    <transition name="fade">
      <div v-if="saving && !loading" class="top-progress"></div>
    </transition>

    <main class="content">
      <PageLoader
        v-if="loading"
        title="正在读取本地配置"
        description="加载配置、隧道与运行状态…"
      />
      <div v-else-if="loadError" class="load-error card">
        <EmptyState
          title="配置加载失败"
          :description="loadError"
        >
          <template #icon>
            <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.6">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 8v5M12 16h.01" stroke-linecap="round" />
            </svg>
          </template>
          <template #action>
            <button class="btn btn-primary" @click="loadAppData">重新加载</button>
          </template>
        </EmptyState>
      </div>
      <transition v-else name="view" appear>
        <component
          :is="viewMap[activeView as keyof typeof viewMap]"
          :key="activeView"
          :profile="activeProfile"
          :profiles="profiles"
          :active-id="activeId"
          :logs="logs"
          :user="user"
          :theme="theme"
          :frp-running="frpRunning"
          @add="addTunnel"
          @update-tunnel="updateTunnel"
          @remove="removeTunnel"
          @remove-many="removeTunnels"
          @set-enabled-many="setTunnelsEnabled"
          @toggle="toggleTunnel"
          @update-profile="updateProfile"
          @replace-profile="replaceProfile"
          @create-profile="createProfile"
          @delete-profile="deleteProfile"
          @delete-profiles="deleteProfiles"
          @duplicate-profile="duplicateProfile"
          @activate-profile="activateProfile"
          @profiles-changed="reloadProfiles"
          @go-dashboard="activeView = 'dashboard'"
          @add-log="addLog"
          @clear="clearLogs"
          @update-user="updateUser"
          @set-theme="setTheme"
        />
      </transition>
    </main>
    <!-- 关闭确认 -->
    <transition name="fade">
      <div v-if="showCloseDialog" class="close-mask">
        <div class="close-dialog">
          <div class="close-head">
            <div class="close-icon">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a4 4 0 0 0 0-8z" />
              </svg>
            </div>
            <div>
              <h3>关闭 NetTool</h3>
              <p class="muted small">选择后台运行时，已连接的隧道会继续保持，可从托盘图标重新打开主界面。</p>
            </div>
          </div>

          <label class="remember">
            <input type="checkbox" v-model="rememberClose" />
            <span class="small">记住我的选择（之后可在「设置」中修改）</span>
          </label>

          <div class="close-actions">
            <button class="btn btn-ghost" @click="cancelClose">取消</button>
            <button class="btn btn-ghost" @click="resolveClose('quit')">直接关闭</button>
            <button class="btn btn-primary" @click="resolveClose('tray')">后台运行</button>
          </div>
        </div>
      </div>
    </transition>
    <ToastHost />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  height: 100%;
  width: 100%;
  /* 外壳固定不参与页面切换时的重排 */
  contain: layout paint;
}
.content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 24px 28px;
  background: var(--bg);
  /* 切换页面时避免整页重排，减少模糊层重新栅格化造成的「顿挫」 */
  contain: layout paint style;
}
.load-error {
  max-width: 620px;
  margin: 8vh auto;
  padding: 8px;
}
.loading {
  padding: 40px 0;
  text-align: center;
}
@media (max-width: 768px) {
  .content {
    padding: 16px;
  }
}

/* 关闭确认弹窗 */
.close-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(3px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 300;
}
.close-dialog {
  width: 460px;
  max-width: 92vw;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
  padding: 22px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.close-head {
  display: flex;
  gap: 14px;
  align-items: flex-start;
}
.close-icon {
  width: 40px;
  height: 40px;
  border-radius: 11px;
  background: var(--primary-soft);
  color: var(--primary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.close-head h3 {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 4px;
}
.close-head p {
  line-height: 1.6;
}
.remember {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  color: var(--text-secondary);
}
.remember input {
  width: 15px;
  height: 15px;
  accent-color: var(--primary);
}
.close-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
