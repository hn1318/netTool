<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import PasswordField from "../components/PasswordField.vue";
import Spinner from "../components/Spinner.vue";
import Skeleton from "../components/Skeleton.vue";
import EmptyState from "../components/EmptyState.vue";
import { toast } from "../composables/toast";
import {
  providerList,
  providerAccounts,
  providerLoginKey,
  providerLoginPassword,
  providerWebLoginStart,
  providerWebLoginPoll,
  providerWebLoginCancel,
  providerSign,
  providerLogout,
  providerUser,
  providerTunnels,
  providerConfig,
  providerImport,
  providerKick,
  providerToggle,
  providerCaptchaInfo,
  providerDecodeCaptcha,
  openExternal,
  startFrpc,
} from "../api";
import type {
  ProviderAccount,
  ProviderMeta,
  ProviderTunnel,
  ProviderUser,
  WebLoginStart,
} from "../types";

const emit = defineEmits<{
  (e: "profiles-changed"): void;
  (e: "go-dashboard"): void;
}>();

const metas = ref<ProviderMeta[]>([]);
const accounts = ref<Record<string, ProviderAccount>>({});
const current = ref("mefrp");

const user = ref<ProviderUser | null>(null);
const tunnels = ref<ProviderTunnel[]>([]);
const selected = ref<string[]>([]);

const loginMode = ref<"key" | "password" | "web">("key");
const keyInput = ref("");
const username = ref("");
const password = ref("");
const captcha = ref("");
const captchaUrl = ref("");
const signCaptcha = ref("");
const showSign = ref(false);

const busy = ref("");
const error = ref("");
const info = ref("");

const loadingMetas = ref(true);
const loadingAccounts = ref(true);
const loadingData = ref(false);

// 网页登录状态
const webLogin = ref<WebLoginStart | null>(null);
const webWaiting = ref(false);
const webAttempts = ref(0);
let webTimer: number | undefined;

const previewText = ref("");
const showPreview = ref(false);
const importName = ref("");

const meta = computed<ProviderMeta | undefined>(() =>
  metas.value.find((m) => m.id === current.value)
);
const account = computed<ProviderAccount | undefined>(() => accounts.value[current.value]);
const loggedIn = computed(() => !!account.value);
const canPassword = computed(() => !!meta.value?.loginModes.includes("password"));
const canWeb = computed(() => !!meta.value?.loginModes.includes("web"));

const keyHint = computed(() => {
  switch (current.value) {
    case "mefrp":
      return "在 ME Frp 控制台 · 账户设置中获取访问密钥（即 API Token）";
    case "openfrp":
      return "旧版 Authorization（以 OPENFRP 开头）；新账号请使用「网页登录」";
    case "chmlfrp":
      return "在 ChmlFrp 面板「用户中心」复制访问令牌（access_token）";
    case "sakura":
      return "账户信息 → 访问密钥（需完成实名认证）";
    case "msl":
      return "MSL 用户中心复制的 Token（Authorization: Bearer <Token> 中的 Token）";
    default:
      return "访问密钥";
  }
});

const selectedTunnels = computed(() =>
  tunnels.value.filter((t) => selected.value.includes(t.id))
);
const nodeCount = computed(
  () => new Set(selectedTunnels.value.map((t) => t.node)).size
);
const nodeSummary = computed(() =>
  [...new Set(selectedTunnels.value.map((t) => t.node))].join("、")
);

function clearMessages() {
  error.value = "";
  info.value = "";
}

async function loadMetas() {
  loadingMetas.value = true;
  try {
    metas.value = await providerList();
  } catch (e) {
    error.value = String(e);
  } finally {
    loadingMetas.value = false;
  }
}

async function loadAccounts() {
  loadingAccounts.value = true;
  try {
    const list = await providerAccounts();
    const map: Record<string, ProviderAccount> = {};
    for (const a of list) map[a.provider] = a;
    accounts.value = map;
  } catch (e) {
    console.error(e);
  } finally {
    loadingAccounts.value = false;
  }
}

async function loadProvider() {
  clearMessages();
  busy.value = "refresh";
  loadingData.value = true;
  try {
    const [u, t] = await Promise.all([
      providerUser(current.value),
      providerTunnels(current.value),
    ]);
    user.value = u;
    tunnels.value = t;
    selected.value = selected.value.filter((id) => t.some((x) => x.id === id));
  } catch (e) {
    error.value = String(e);
    user.value = null;
    tunnels.value = [];
  } finally {
    busy.value = "";
    loadingData.value = false;
  }
}

function defaultLoginMode(id: string): "key" | "password" | "web" {
  const m = metas.value.find((x) => x.id === id);
  if (m?.loginModes.includes("web")) return "web";
  return "key";
}

async function switchProvider(id: string) {
  stopWebLogin();
  current.value = id;
  user.value = null;
  tunnels.value = [];
  selected.value = [];
  captchaUrl.value = "";
  showSign.value = false;
  signCaptcha.value = "";
  loginMode.value = defaultLoginMode(id);
  clearMessages();
  if (accounts.value[id]) {
    await loadProvider();
  }
}

// ============ 网页登录（OpenFrp Remote Login） ============

function clearWebTimer() {
  if (webTimer) {
    clearTimeout(webTimer);
    webTimer = undefined;
  }
}

function resetWebState() {
  clearWebTimer();
  webWaiting.value = false;
  webLogin.value = null;
  webAttempts.value = 0;
}

async function stopWebLogin(cancel = true) {
  const uuid = webLogin.value?.requestUuid;
  resetWebState();
  if (cancel && uuid) {
    try {
      await providerWebLoginCancel(uuid);
    } catch {
      /* 忽略 */
    }
  }
}

async function pollWebLogin() {
  const session = webLogin.value;
  if (!session || !webWaiting.value) return;
  webAttempts.value += 1;
  // 5 秒 / 次，最多 5 分钟
  if (webAttempts.value > 60) {
    await stopWebLogin(false);
    error.value = "授权等待超时，请重新点击「网页登录」";
    return;
  }
  try {
    const u = await providerWebLoginPoll(current.value, session.requestUuid);
    if (u) {
      resetWebState();
      await loadAccounts();
      await loadProvider();
      info.value = `已登录：${u.username || meta.value?.name}`;
      return;
    }
    webTimer = window.setTimeout(pollWebLogin, 5000);
  } catch (e) {
    resetWebState();
    error.value = String(e);
  }
}

async function startWebLogin() {
  clearMessages();
  if (webWaiting.value) return;
  busy.value = "web-login";
  try {
    const session = await providerWebLoginStart(current.value);
    webLogin.value = session;
    webWaiting.value = true;
    webAttempts.value = 0;
    await openExternal(session.url);
    info.value = "已打开 OpenFrp 授权页面，请在浏览器中确认登录";
    webTimer = window.setTimeout(pollWebLogin, 3000);
  } catch (e) {
    resetWebState();
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function openCaptcha() {
  try {
    if (!captchaUrl.value) {
      captchaUrl.value = (await providerCaptchaInfo(current.value)).url;
    }
    await openExternal(captchaUrl.value);
  } catch (e) {
    error.value = String(e);
  }
}

async function parseCaptcha() {
  try {
    captcha.value = await providerDecodeCaptcha(captcha.value);
    info.value = "验证码已解析";
  } catch (e) {
    error.value = String(e);
  }
}

async function loginByKey() {
  clearMessages();
  if (!keyInput.value.trim()) {
    error.value = "请输入凭据";
    return;
  }
  busy.value = "login";
  try {
    const u = await providerLoginKey(current.value, keyInput.value.trim());
    keyInput.value = "";
    await loadAccounts();
    await loadProvider();
    info.value = `已登录：${u.username || meta.value?.name}`;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function loginByPassword() {
  clearMessages();
  if (!username.value.trim() || !password.value) {
    error.value = "请填写用户名与密码";
    return;
  }
  if (meta.value?.captcha && !captcha.value.trim()) {
    error.value = "请先完成人机验证并粘贴验证码";
    return;
  }
  busy.value = "login";
  try {
    const u = await providerLoginPassword(
      current.value,
      username.value.trim(),
      password.value,
      captcha.value
    );
    password.value = "";
    captcha.value = "";
    await loadAccounts();
    await loadProvider();
    info.value = `已登录：${u.username || meta.value?.name}`;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function doSign() {
  clearMessages();
  if (!signCaptcha.value.trim()) {
    error.value = "请先完成人机验证并粘贴验证码";
    return;
  }
  busy.value = "sign";
  try {
    const msg = await providerSign(current.value, signCaptcha.value.trim());
    signCaptcha.value = "";
    showSign.value = false;
    info.value = msg;
    await loadProvider();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function logout() {
  clearMessages();
  await stopWebLogin();
  try {
    await providerLogout(current.value);
    await loadAccounts();
    user.value = null;
    tunnels.value = [];
    selected.value = [];
    info.value = "已退出登录";
  } catch (e) {
    error.value = String(e);
  }
}

function toggleSelect(t: ProviderTunnel) {
  const i = selected.value.indexOf(t.id);
  if (i >= 0) selected.value.splice(i, 1);
  else selected.value.push(t.id);
}

function selectAll() {
  const selectable = tunnels.value.filter((t) => !t.banned);
  selected.value =
    selected.value.length === selectable.length ? [] : selectable.map((t) => t.id);
}

async function doPreview() {
  clearMessages();
  if (!selected.value.length) {
    error.value = "请先选择隧道";
    return;
  }
  busy.value = "preview";
  try {
    previewText.value = await providerConfig(current.value, selected.value);
    showPreview.value = true;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function copyPreview() {
  try {
    await navigator.clipboard.writeText(previewText.value);
    info.value = "配置已复制";
  } catch {
    /* 忽略 */
  }
}

async function downloadPreview() {
  const blob = new Blob([previewText.value], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${importName.value || current.value}.toml`;
  a.click();
  URL.revokeObjectURL(url);
}

async function doImport(start: boolean) {
  clearMessages();
  if (!selected.value.length) {
    error.value = "请先选择隧道";
    return;
  }
  if (nodeCount.value > 1) {
    error.value = `所选隧道分布在 ${nodeCount.value} 个节点，请按节点分批导入`;
    return;
  }
  busy.value = start ? "import-start" : "import";
  try {
    const profile = await providerImport(
      current.value,
      selected.value,
      importName.value.trim() || undefined
    );
    emit("profiles-changed");
    info.value = `已导入为配置「${profile.name}」，共 ${profile.tunnels.length} 条隧道`;
    if (start) {
      await startFrpc(profile);
      emit("go-dashboard");
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function kick(t: ProviderTunnel) {
  clearMessages();
  busy.value = `kick-${t.id}`;
  try {
    await providerKick(current.value, t.id);
    info.value = `已强制下线「${t.name}」`;
    await loadProvider();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

async function toggleDisabled(t: ProviderTunnel) {
  clearMessages();
  busy.value = `toggle-${t.id}`;
  try {
    await providerToggle(current.value, t.id, !t.disabled);
    await loadProvider();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
  }
}

function statusText(t: ProviderTunnel) {
  if (t.banned) return "封禁";
  if (t.online) return "在线";
  if (t.disabled) return "已禁用";
  return "离线";
}

function statusClass(t: ProviderTunnel) {
  if (t.banned) return "err";
  if (t.online) return "ok";
  return "idle";
}

onMounted(async () => {
  await loadMetas();
  await loadAccounts();
  const firstLogged = metas.value.find((m) => accounts.value[m.id]);
  await switchProvider(firstLogged?.id || metas.value[0]?.id || "mefrp");
});

onBeforeUnmount(() => {
  stopWebLogin();
});

// 统一把操作结果转成右下角通知
watch(error, (v) => {
  if (v) toast.error("操作失败", v);
});
watch(info, (v) => {
  if (v) toast.success(v);
});
</script>

<template>
  <div class="providers">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>第三方服务</h2>
        <p class="muted small">登录 FRP 服务商后读取远端隧道，一键导入为本地配置并启动</p>
      </div>
      <button class="btn btn-ghost" :disabled="!loggedIn || busy === 'refresh'" @click="loadProvider">
        <Spinner v-if="busy === 'refresh'" :size="13" :width="2" />
        {{ busy === "refresh" ? "刷新中..." : "刷新" }}
      </button>
    </div>

    <transition name="fade">
      <div v-if="error" class="msg error">{{ error }}</div>
    </transition>
    <transition name="fade">
      <div v-if="info && !error" class="msg ok">{{ info }}</div>
    </transition>

    <!-- 服务商选择（读取中显示骨架） -->
    <div v-if="loadingMetas" class="provider-tabs">
      <div v-for="n in 4" :key="`sk-tab-${n}`" class="card provider-tab sk-tab">
        <div class="tab-head">
          <Skeleton width="74px" height="15px" />
          <Skeleton width="52px" height="18px" radius="999px" />
        </div>
        <Skeleton width="58%" height="11px" />
        <Skeleton width="72%" height="11px" />
      </div>
    </div>
    <div v-else class="provider-tabs">
      <button
        v-for="m in metas"
        :key="m.id"
        class="card provider-tab"
        :class="{ active: m.id === current }"
        @click="switchProvider(m.id)"
      >
        <div class="tab-head">
          <span class="tab-name">{{ m.name }}</span>
          <span class="badge" :class="accounts[m.id] ? 'badge-on' : 'badge-off'">
            {{ loadingAccounts ? "读取中" : accounts[m.id] ? "已登录" : "未登录" }}
          </span>
        </div>
        <div class="tab-site muted small">{{ m.site.replace("https://", "") }}</div>
        <div class="tab-modes">
          <span v-if="m.loginModes.includes('key')" class="mode">密钥</span>
          <span v-if="m.loginModes.includes('web')" class="mode">网页</span>
          <span v-if="m.loginModes.includes('password')" class="mode">密码</span>
          <span v-if="m.captcha" class="mode warn">人机验证</span>
        </div>
        <div v-if="accounts[m.id]" class="tab-account muted small">
          {{ accounts[m.id].username }} · …{{ accounts[m.id].tokenTail }}
        </div>
      </button>
    </div>

    <div class="workspace">
      <!-- 左：登录 / 账户 -->
      <aside class="side">
        <div v-if="loadingMetas || (loadingAccounts && !loggedIn)" class="card panel">
          <Skeleton width="46%" height="17px" />
          <Skeleton width="100%" height="11px" />
          <Skeleton width="82%" height="11px" />
          <div class="sk-form">
            <Skeleton width="38%" height="11px" />
            <Skeleton width="100%" height="38px" radius="8px" />
            <Skeleton width="100%" height="38px" radius="8px" />
          </div>
        </div>

        <div v-else-if="!loggedIn" class="card panel">
          <h3 class="panel-title">登录 {{ meta?.name }}</h3>
          <p class="muted small panel-note">{{ meta?.note }}</p>

          <div class="tabs">
            <button
              v-if="canWeb"
              class="tab"
              :class="{ active: loginMode === 'web' }"
              @click="loginMode = 'web'"
            >
              网页登录
            </button>
            <button class="tab" :class="{ active: loginMode === 'key' }" @click="loginMode = 'key'">
              密钥登录
            </button>
            <button
              v-if="canPassword"
              class="tab"
              :class="{ active: loginMode === 'password' }"
              @click="loginMode = 'password'"
            >
              密码登录
            </button>
          </div>

          <div v-if="loginMode === 'key'" class="form">
            <div class="field">
              <label>访问凭据</label>
              <PasswordField v-model="keyInput" :placeholder="keyHint" />
            </div>
            <button class="btn btn-primary" :disabled="busy === 'login'" @click="loginByKey">
              <Spinner v-if="busy === 'login'" :size="14" :width="2" />
              {{ busy === "login" ? "登录中..." : "登录" }}
            </button>
            <p v-if="current === 'openfrp'" class="muted small">
              OpenFrp 已移除旧版授权凭据复制入口；面板里的 32 位「用户访问密钥」不能用于 API 鉴权，请优先使用「网页登录」。
            </p>
          </div>

          <div v-else-if="loginMode === 'password'" class="form">
            <div class="field">
              <label>用户名或邮箱</label>
              <input class="input" v-model="username" />
            </div>
            <div class="field">
              <label>密码</label>
              <PasswordField v-model="password" />
            </div>
            <div v-if="meta?.captcha" class="captcha-box">
              <div class="captcha-head">
                <span class="section-label">人机验证</span>
                <button class="btn btn-ghost btn-xs" @click="openCaptcha">打开验证页面</button>
              </div>
              <ol class="steps muted small">
                <li>打开验证页面并完成人机验证</li>
                <li>复制页面给出的验证码（Base64 文本）</li>
                <li>粘贴到下方并登录</li>
              </ol>
              <div class="captcha-input">
                <input class="input" v-model="captcha" placeholder="粘贴验证码" />
                <button class="btn btn-ghost btn-xs" :disabled="!captcha" @click="parseCaptcha">
                  解析
                </button>
              </div>
            </div>
            <button class="btn btn-primary" :disabled="busy === 'login'" @click="loginByPassword">
              <Spinner v-if="busy === 'login'" :size="14" :width="2" />
              {{ busy === "login" ? "登录中..." : "登录" }}
            </button>
          </div>

          <div v-else class="form">
            <p class="muted small web-note">
              点击下方按钮后会在浏览器打开 OpenFrp 官方授权页；在页面中确认登录即可，程序每 5 秒自动检测一次授权结果。
            </p>
            <button
              class="btn btn-primary"
              :disabled="webWaiting || busy === 'web-login'"
              @click="startWebLogin"
            >
              <Spinner v-if="webWaiting || busy === 'web-login'" :size="14" :width="2" />
              {{ webWaiting ? "等待浏览器授权..." : busy === "web-login" ? "正在申请授权..." : "打开授权页面并登录" }}
            </button>

            <div v-if="webWaiting" class="web-wait card">
              <div class="web-wait-head">
                <Spinner :size="16" :width="2" />
                <span class="small">已等待 {{ webAttempts * 5 }} 秒</span>
              </div>
              <p class="muted small">请在浏览器中完成 OpenFrp 登录，成功后本页会自动读取账号与隧道。</p>
              <div class="flex gap-2">
                <button
                  v-if="webLogin"
                  class="btn btn-ghost btn-xs"
                  @click="openExternal(webLogin.url)"
                >
                  重新打开授权页
                </button>
                <button class="btn btn-ghost btn-xs" @click="stopWebLogin()">取消</button>
              </div>
            </div>

            <p class="muted small">
              授权有效期由 OpenFrp 控制；若长时间未完成，可取消后重新发起。
            </p>
          </div>
        </div>

        <div v-else class="card panel">
          <div v-if="loadingData && !user" class="user-head">
            <Skeleton width="40px" height="40px" circle />
            <div class="user-main">
              <Skeleton width="120px" height="15px" />
              <Skeleton width="80%" height="11px" />
            </div>
          </div>
          <div v-else class="user-head">
            <div class="avatar">
              {{ (user?.username || account?.username || "?").slice(0, 1).toUpperCase() }}
            </div>
            <div class="user-main">
              <div class="user-name">{{ user?.username || account?.username }}</div>
              <div class="muted small">{{ user?.email || account?.email || meta?.name }}</div>
            </div>
          </div>

          <div v-if="loadingData && !user" class="user-stats">
            <Skeleton width="100%" height="13px" />
            <Skeleton width="100%" height="13px" />
            <Skeleton width="100%" height="13px" />
          </div>
          <template v-else>
            <div class="user-badges">
              <span v-if="user?.group" class="badge badge-group">{{ user.group }}</span>
              <span class="badge badge-method">
                {{
                  account?.loginMethod === "password"
                    ? "密码登录"
                    : account?.loginMethod === "web"
                    ? "网页登录"
                    : "密钥登录"
                }}
              </span>
            </div>

            <div class="user-stats">
              <div class="stat">
                <span class="muted small">隧道配额</span>
                <strong>{{ user?.tunnelUsed ?? "-" }} / {{ user?.tunnelLimit || "-" }}</strong>
              </div>
              <div class="stat">
                <span class="muted small">流量</span>
                <strong>{{ user?.traffic || "-" }}</strong>
              </div>
              <div class="stat">
                <span class="muted small">今日签到</span>
                <strong>
                  {{
                    user?.signed === null || user?.signed === undefined
                      ? "-"
                      : user.signed
                      ? "已完成"
                      : "未签到"
                  }}
                </strong>
              </div>
            </div>

            <div v-if="user?.extras?.length" class="extras">
              <div v-for="kv in user.extras" :key="kv.label" class="extra-item">
                <span class="muted small">{{ kv.label }}</span>
                <span class="small">{{ kv.value }}</span>
              </div>
            </div>

            <div v-if="current === 'mefrp'" class="sign-block">
              <div v-if="user?.signed" class="muted small">今日已签到</div>
              <template v-else>
                <div class="sign-head">
                  <span class="muted small">签到可领取流量</span>
                  <button class="btn btn-ghost btn-xs" :disabled="busy === 'sign'" @click="openCaptcha(); showSign = true">
                    签到
                  </button>
                </div>
                <div v-if="showSign" class="captcha-input">
                  <input class="input" v-model="signCaptcha" placeholder="粘贴人机验证码" />
                  <button class="btn btn-primary btn-xs" :disabled="busy === 'sign'" @click="doSign">
                    {{ busy === "sign" ? "提交中" : "提交" }}
                  </button>
                </div>
              </template>
            </div>
          </template>

          <div class="side-actions">
            <button class="btn btn-ghost btn-sm" :disabled="busy === 'refresh'" @click="loadProvider">
              <Spinner v-if="busy === 'refresh'" :size="12" :width="2" />
              {{ busy === "refresh" ? "刷新中" : "刷新信息" }}
            </button>
            <button class="btn btn-danger btn-sm" @click="logout">退出登录</button>
          </div>
        </div>
      </aside>

      <!-- 右：隧道 -->
      <section class="main">
        <div v-if="!loggedIn" class="card empty-card">
          <EmptyState
            :title="loadingMetas ? '正在读取服务商' : `登录 ${meta?.name || '服务商'} 后管理远端隧道`"
            description="登录后即可读取远端隧道、预览配置，并一键导入为本地配置启动"
          >
            <template #icon>
              <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.6">
                <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a4 4 0 0 0 0-8z" />
              </svg>
            </template>
          </EmptyState>
          <p class="muted small">已支持 ME Frp、OpenFrp、ChmlFrp、SakuraFrp、MSL Frp，后续继续扩展</p>
        </div>

        <div v-else class="card list-card">
          <div class="list-head">
            <div class="list-title">
              <h3 class="card-title">远端隧道</h3>
              <span v-if="loadingData" class="muted small sk-inline">
                <Spinner :size="12" :width="2" />
                正在读取远端隧道…
              </span>
              <span v-else class="muted small">
                共 {{ tunnels.length }} 条 · 已选 {{ selected.length }} 条
                <template v-if="nodeSummary"> · 节点：{{ nodeSummary }}</template>
              </span>
            </div>
            <div class="list-actions">
              <button class="btn btn-ghost btn-sm" @click="selectAll">全选 / 取消</button>
              <button class="btn btn-ghost btn-sm" :disabled="busy === 'preview'" @click="doPreview">
                <Spinner v-if="busy === 'preview'" :size="12" :width="2" />
                {{ busy === "preview" ? "生成中..." : "预览配置" }}
              </button>
              <button class="btn btn-primary btn-sm" :disabled="!!busy" @click="doImport(false)">
                <Spinner v-if="busy === 'import'" :size="12" :width="2" />
                {{ busy === "import" ? "导入中..." : "导入配置" }}
              </button>
              <button class="btn btn-success btn-sm" :disabled="!!busy" @click="doImport(true)">
                <Spinner v-if="busy === 'import-start'" :size="12" :width="2" />
                {{ busy === "import-start" ? "启动中..." : "导入并连接" }}
              </button>
            </div>
          </div>

          <div class="table-wrap">
            <table class="proxy-table">
              <thead>
                <tr>
                  <th class="col-check"></th>
                  <th>隧道</th>
                  <th>类型</th>
                  <th>本地</th>
                  <th>远端</th>
                  <th>速率</th>
                  <th>节点</th>
                  <th>状态</th>
                  <th v-if="current === 'mefrp'" class="ops-col">操作</th>
                </tr>
              </thead>
              <tbody>
                <template v-if="loadingData">
                  <tr v-for="n in 4" :key="`sk-tunnel-${n}`">
                    <td class="col-check"><Skeleton width="15px" height="15px" radius="4px" /></td>
                    <td><Skeleton width="120px" height="12px" /></td>
                    <td><Skeleton width="48px" height="18px" radius="999px" /></td>
                    <td><Skeleton width="110px" height="12px" /></td>
                    <td><Skeleton width="130px" height="12px" /></td>
                    <td><Skeleton width="64px" height="12px" /></td>
                    <td><Skeleton width="90px" height="12px" /></td>
                    <td><Skeleton width="46px" height="18px" radius="999px" /></td>
                    <td v-if="current === 'mefrp'"><Skeleton width="72px" height="12px" /></td>
                  </tr>
                </template>
                <template v-else>
                  <tr v-for="t in tunnels" :key="t.id">
                  <td class="col-check">
                    <input
                      type="checkbox"
                      :checked="selected.includes(t.id)"
                      :disabled="t.banned"
                      @change="toggleSelect(t)"
                    />
                  </td>
                  <td class="name">
                    {{ t.name }}
                    <div class="muted small">ID {{ t.id }}</div>
                  </td>
                  <td><span class="badge type-badge">{{ t.proxyType.toUpperCase() }}</span></td>
                  <td class="mono small">{{ t.localIp }}:{{ t.localPort }}</td>
                  <td class="mono small">{{ t.remoteDesc }}</td>
                  <td class="small">{{ t.speed || "-" }}</td>
                  <td class="small">{{ t.node || "-" }}</td>
                  <td>
                    <span class="status-tag" :class="statusClass(t)">{{ statusText(t) }}</span>
                  </td>
                  <td v-if="current === 'mefrp'" class="ops">
                    <button
                      class="btn btn-ghost btn-xs"
                      :disabled="busy === `toggle-${t.id}`"
                      @click="toggleDisabled(t)"
                    >
                      {{ t.disabled ? "启用" : "禁用" }}
                    </button>
                    <button
                      class="btn btn-danger btn-xs"
                      :disabled="!t.online || busy === `kick-${t.id}`"
                      @click="kick(t)"
                    >
                      下线
                    </button>
                  </td>
                </tr>
                  <tr v-if="tunnels.length === 0">
                    <td :colspan="current === 'mefrp' ? 9 : 8" class="empty">
                      <EmptyState
                        compact
                        title="暂无远端隧道"
                        description="可先到服务商控制台创建隧道，再回到这里刷新"
                      />
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
          </div>

          <div class="import-row">
            <div class="field" style="max-width: 300px">
              <label>导入后的配置名称（可选）</label>
              <input class="input" v-model="importName" :placeholder="meta?.name" />
            </div>
            <p class="muted small">导入会生成独立配置，可在「配置管理」查看；同一批次需属于同一节点</p>
          </div>
        </div>
      </section>
    </div>

    <!-- 配置预览 -->
    <transition name="modal">
      <div v-if="showPreview" class="modal-mask" @click.self="showPreview = false">
        <div class="modal">
          <div class="modal-header">
            <h3>配置预览（frpc.toml）</h3>
            <button class="icon-btn" @click="showPreview = false">✕</button>
          </div>
          <div class="modal-body">
            <pre class="code-block">{{ previewText }}</pre>
          </div>
          <div class="modal-footer">
            <button class="btn btn-ghost" @click="copyPreview">复制</button>
            <button class="btn btn-ghost" @click="downloadPreview">下载</button>
            <button class="btn btn-primary" @click="showPreview = false">关闭</button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.providers {
  animation: fadeUp 0.4s ease;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.msg {
  padding: 9px 14px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  word-break: break-all;
}
.msg.error {
  background: rgba(248, 113, 113, 0.12);
  border: 1px solid rgba(248, 113, 113, 0.25);
  color: var(--danger);
}
.msg.ok {
  background: rgba(52, 211, 153, 0.12);
  border: 1px solid rgba(52, 211, 153, 0.25);
  color: var(--success);
}
.provider-tabs {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(196px, 1fr));
  gap: 12px;
}
.provider-tab {
  color: var(--text);
  text-align: left;
  padding: 14px 16px;
  border-radius: var(--radius);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 6px;
  transition: all var(--transition);
}
.provider-tab:hover {
  border-color: var(--border-light);
  transform: translateY(-2px);
}
.provider-tab.active {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary-soft), var(--shadow);
}
.sk-tab {
  gap: 10px;
  cursor: default;
  min-height: 108px;
}
.sk-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 6px;
}
.sk-inline {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.web-note {
  line-height: 1.7;
}
.web-wait {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-color: var(--border-light);
  background: var(--bg-elevated);
}
.web-wait-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.tab-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.tab-name {
  font-size: 15px;
  font-weight: 700;
}
.badge-on {
  background: rgba(52, 211, 153, 0.14);
  color: var(--success);
}
.badge-off {
  background: var(--bg-hover);
  color: var(--text-muted);
}
.tab-site {
  font-family: "JetBrains Mono", "Fira Code", monospace;
}
.tab-modes {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.mode {
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--primary-soft);
  color: var(--primary);
}
.mode.warn {
  background: rgba(251, 191, 36, 0.14);
  color: var(--warning);
}
.tab-account {
  border-top: 1px dashed var(--border);
  padding-top: 6px;
}
.workspace {
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  gap: 14px;
  align-items: start;
}
.side {
  display: flex;
  flex-direction: column;
  gap: 14px;
  position: sticky;
  top: 0;
}
.panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.panel-title {
  font-size: 15px;
  font-weight: 600;
}
.panel-note {
  line-height: 1.6;
}
.tabs {
  display: flex;
  gap: 6px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border);
}
.tab {
  padding: 5px 12px;
  border-radius: 999px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 500;
}
.tab.active {
  background: var(--primary-soft);
  color: var(--primary);
}
.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.captcha-box {
  background: var(--bg-elevated);
  border: 1px dashed var(--border-light);
  border-radius: var(--radius-sm);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.captcha-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
}
.steps {
  margin: 0;
  padding-left: 16px;
  line-height: 1.7;
}
.captcha-input {
  display: flex;
  gap: 8px;
}
.user-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.avatar {
  width: 40px;
  height: 40px;
  border-radius: 11px;
  background: var(--primary-soft);
  color: var(--primary);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 17px;
  font-weight: 700;
  flex-shrink: 0;
}
.user-main {
  min-width: 0;
}
.user-name {
  font-size: 15px;
  font-weight: 600;
}
.user-badges {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.badge-group {
  background: rgba(167, 139, 250, 0.14);
  color: #a78bfa;
}
.badge-method {
  background: var(--bg-hover);
  color: var(--text-secondary);
}
.user-stats {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  background: var(--bg-elevated);
  border-radius: var(--radius-sm);
}
.stat {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}
.stat strong {
  font-size: 13px;
  text-align: right;
  word-break: break-all;
}
.extras {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.extra-item {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}
.sign-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  background: var(--bg-elevated);
  border-radius: var(--radius-sm);
}
.sign-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.side-actions {
  display: flex;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid var(--border);
}
.main {
  min-width: 0;
}
.list-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.list-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.card-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 2px;
}
.list-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.table-wrap {
  overflow-x: auto;
}
.proxy-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.proxy-table th {
  text-align: left;
  font-weight: 500;
  font-size: 12px;
  color: var(--text-muted);
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
}
.proxy-table td {
  padding: 9px 10px;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}
.proxy-table td.small {
  white-space: nowrap;
}
.proxy-table tr:last-child td {
  border-bottom: none;
}
.col-check {
  width: 34px;
  text-align: center;
}
.col-check input {
  width: 15px;
  height: 15px;
  accent-color: var(--primary);
}
.name {
  font-weight: 600;
}
.mono {
  font-family: "JetBrains Mono", "Fira Code", monospace;
}
.type-badge {
  background: var(--bg-hover);
  color: var(--text-secondary);
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
.ops {
  white-space: nowrap;
}
.import-row {
  display: flex;
  align-items: flex-end;
  gap: 16px;
  flex-wrap: wrap;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}
.empty-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 60px 20px;
  text-align: center;
}
.empty-icon {
  width: 54px;
  height: 54px;
  border-radius: 14px;
  background: var(--primary-soft);
  color: var(--primary);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 4px;
}
.empty {
  text-align: center;
  padding: 30px 0;
}
.btn-sm {
  padding: 5px 10px;
  font-size: 12px;
}
.btn-xs {
  padding: 3px 8px;
  font-size: 11px;
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
  width: 760px;
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
.code-block {
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 14px;
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-secondary);
  overflow: auto;
  max-height: 50vh;
  white-space: pre;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
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
@media (max-width: 1180px) {
  .workspace {
    grid-template-columns: 1fr;
  }
  .side {
    position: static;
  }
}
</style>
