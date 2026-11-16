<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import Spinner from "../components/Spinner.vue";
import Skeleton from "../components/Skeleton.vue";
import { toast } from "../composables/toast";
import {
  appearance,
  updateAppearance,
  imageToDataUrl,
  DEFAULT_APPEARANCE,
} from "../appearance";
import {
  getAppDataDir,
  getInstalledCore,
  revealPath,
  getAppSettings,
  saveAppSettings,
  getStorageInfo,
  pickDirectory,
  applyStorageSettings,
  getAutostart,
  setAutostart,
} from "../api";
import type { InstalledCore, AppSettings, StorageInfo } from "../types";

defineProps<{ theme?: string; profile?: unknown }>();
const emit = defineEmits<{
  (e: "set-theme", t: string): void;
}>();

// ============ 运行环境 ============
const core = ref<InstalledCore | null>(null);
const appDataDir = ref("");
const loadingEnv = ref(true);

// ============ 存储位置 ============
const storage = ref<StorageInfo | null>(null);
const dataDirInput = ref("");
const logDirInput = ref("");
const migrate = ref(true);
const applyingStorage = ref(false);
const loadingStorage = ref(true);

// ============ 关闭行为 ============
const appSettings = ref<AppSettings>({ closeBehavior: "ask" });
const closeOptions = [
  { value: "ask", label: "每次询问", desc: "关闭时弹窗让你选择后台运行还是直接退出" },
  { value: "tray", label: "后台运行", desc: "关闭窗口后隐藏到托盘，隧道继续运行" },
  { value: "quit", label: "直接退出", desc: "关闭窗口即结束程序，并停止所有隧道" },
] as const;

// ============ 开机自启动 ============
const autoStart = ref(false);
const autoStartBusy = ref(false);

// ============ 外观 ============
const bgInput = ref<HTMLInputElement | null>(null);
const uploadingBg = ref(false);
const isDefaultBg = computed(() => !appearance.value.bgImage);

function chooseBg() {
  bgInput.value?.click();
}

async function onBgPicked(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    toast.error("请选择图片文件");
    return;
  }
  uploadingBg.value = true;
  try {
    const dataUrl = await imageToDataUrl(file);
    updateAppearance({ bgImage: dataUrl });
    toast.success("背景图片已更新");
  } catch (err) {
    toast.error("设置背景失败", String(err));
  } finally {
    uploadingBg.value = false;
    if (bgInput.value) bgInput.value.value = "";
  }
}

function clearBg() {
  updateAppearance({ bgImage: "" });
  toast.info("已移除背景图片");
}

function resetAppearance() {
  updateAppearance({ ...DEFAULT_APPEARANCE });
  emit("set-theme", "dark");
  toast.info("外观已恢复默认");
}

// ============ 通用 ============
async function changeCloseBehavior(value: AppSettings["closeBehavior"]) {
  appSettings.value = { ...appSettings.value, closeBehavior: value };
  try {
    appSettings.value = await saveAppSettings(appSettings.value);
    toast.success("设置已保存", "关闭窗口行为已更新");
  } catch (e) {
    toast.error("保存失败", String(e));
  }
}

async function applyStorage() {
  applyingStorage.value = true;
  try {
    const msg = await applyStorageSettings({
      dataDir: dataDirInput.value.trim() || null,
      logDir: logDirInput.value.trim() || null,
      migrate: migrate.value,
    });
    toast.success("已保存存储位置", msg);
    const info = await getStorageInfo();
    storage.value = info;
    dataDirInput.value = info.dataDir;
    logDirInput.value = info.logDir;
  } catch (e) {
    toast.error("保存存储位置失败", String(e));
  } finally {
    applyingStorage.value = false;
  }
}

async function chooseDataDir() {
  try {
    const picked = await pickDirectory(dataDirInput.value);
    if (picked) dataDirInput.value = picked;
  } catch (e) {
    toast.error("选择文件夹失败", String(e));
  }
}

async function chooseLogDir() {
  try {
    const picked = await pickDirectory(logDirInput.value);
    if (picked) logDirInput.value = picked;
  } catch (e) {
    toast.error("选择文件夹失败", String(e));
  }
}

function resetDataDir() {
  if (!storage.value) return;
  dataDirInput.value = storage.value.defaultDataDir;
}

function resetLogDir() {
  if (!storage.value) return;
  logDirInput.value = storage.value.defaultLogDir;
}

async function toggleAutostart() {
  if (autoStartBusy.value) return;
  autoStartBusy.value = true;
  try {
    const next = !autoStart.value;
    const msg = await setAutostart(next);
    autoStart.value = next;
    toast.success(next ? "已开启开机自启动" : "已关闭开机自启动", msg);
  } catch (e) {
    toast.error("设置开机自启动失败", String(e));
  } finally {
    autoStartBusy.value = false;
  }
}

async function openDataDir() {
  try {
    await revealPath(dataDirInput.value || storage.value?.dataDir || "");
  } catch {
    /* 忽略 */
  }
}

async function openCoreDir() {
  try {
    if (core.value?.path) await revealPath(core.value.path);
    else await revealPath(appDataDir.value);
  } catch {
    /* 忽略 */
  }
}

onMounted(async () => {
  try {
    appDataDir.value = await getAppDataDir();
  } catch {
    /* 忽略 */
  }
  try {
    core.value = await getInstalledCore();
  } catch {
    /* 忽略 */
  }
  try {
    appSettings.value = await getAppSettings();
  } catch {
    /* 忽略 */
  }
  loadingEnv.value = false;

  try {
    storage.value = await getStorageInfo();
    dataDirInput.value = storage.value.dataDir;
    logDirInput.value = storage.value.logDir;
  } catch (e) {
    console.error(e);
  }
  loadingStorage.value = false;

  try {
    autoStart.value = await getAutostart();
  } catch {
    /* 忽略 */
  }
});
</script>

<template>
  <div class="settings">
    <div class="page-header">
      <h2>设置</h2>
      <p class="muted small">外观、存储位置、启动行为与运行环境</p>
    </div>

    <div class="settings-grid">
      <!-- 外观 -->
      <div class="card section">
        <div class="section-head">
          <h3 class="section-title">外观</h3>
          <button class="btn btn-ghost btn-sm" @click="resetAppearance">恢复默认</button>
        </div>

        <div class="option-row">
          <div>
            <div class="option-label">界面主题</div>
            <div class="muted small">浅色或深色</div>
          </div>
          <div class="segmented">
            <button :class="{ active: theme === 'dark' }" @click="emit('set-theme', 'dark')">深色</button>
            <button :class="{ active: theme === 'light' }" @click="emit('set-theme', 'light')">浅色</button>
          </div>
        </div>

        <div class="option-row">
          <div>
            <div class="option-label">背景图片</div>
            <div class="muted small">支持 JPG / PNG / WebP，自动压缩后保存在本地</div>
          </div>
          <div class="flex gap-2">
            <input ref="bgInput" type="file" accept="image/*" class="hidden-input" @change="onBgPicked" />
            <button class="btn btn-ghost btn-sm" :disabled="uploadingBg" @click="chooseBg">
              <Spinner v-if="uploadingBg" :size="12" :width="2" />
              {{ uploadingBg ? "处理中" : appearance.bgImage ? "更换图片" : "选择图片" }}
            </button>
            <button v-if="!isDefaultBg" class="btn btn-ghost btn-sm" @click="clearBg">移除</button>
          </div>
        </div>

        <div v-if="appearance.bgImage" class="bg-preview">
          <div
            class="bg-preview-stage"
            :class="{
              contain: appearance.bgFit === 'contain',
              stretch: appearance.bgFit === 'stretch',
            }"
          >
            <img :src="appearance.bgImage" alt="背景预览" />
          </div>
          <div class="bg-preview-bar">
            <span class="muted small">显示方式</span>
            <div class="segmented compact">
              <button :class="{ active: appearance.bgFit === 'fill' }" @click="updateAppearance({ bgFit: 'fill' })">
                等比例铺满
              </button>
              <button :class="{ active: appearance.bgFit === 'contain' }" @click="updateAppearance({ bgFit: 'contain' })">
                完整显示
              </button>
              <button :class="{ active: appearance.bgFit === 'stretch' }" @click="updateAppearance({ bgFit: 'stretch' })">
                拉伸
              </button>
            </div>
            <span class="muted small bg-mask-label">遮罩浓度</span>
            <input
              class="bg-mask-range"
              type="range"
              min="0"
              max="0.8"
              step="0.05"
              :value="appearance.bgMask"
              @input="updateAppearance({ bgMask: Number(($event.target as HTMLInputElement).value) })"
            />
            <span class="value-pill">{{ Math.round(appearance.bgMask * 100) }}%</span>
          </div>
        </div>

        <div class="option-row slider-row">
          <div>
            <div class="option-label">圆角尺寸</div>
            <div class="muted small">0 为直角，最大 24px</div>
          </div>
          <div class="slider-control">
            <input
              type="range"
              min="0"
              max="24"
              step="1"
              :value="appearance.radius"
              @input="updateAppearance({ radius: Number(($event.target as HTMLInputElement).value) })"
            />
            <span class="value-pill">{{ appearance.radius }}px</span>
          </div>
        </div>

        <div class="option-row">
          <div>
            <div class="option-label">磨砂玻璃</div>
            <div class="muted small">卡片与侧栏使用半透明模糊效果</div>
          </div>
          <label class="switch">
            <input
              type="checkbox"
              :checked="appearance.glass"
              @change="updateAppearance({ glass: ($event.target as HTMLInputElement).checked })"
            />
            <span class="slider-switch"></span>
          </label>
        </div>

        <div class="option-row slider-row" :class="{ disabled: !appearance.glass }">
          <div>
            <div class="option-label">模糊强度</div>
            <div class="muted small">高斯模糊半径，越大越接近亚克力磨砂</div>
          </div>
          <div class="slider-control">
            <input
              type="range"
              min="0"
              max="60"
              step="1"
              :disabled="!appearance.glass"
              :value="appearance.blur"
              @input="updateAppearance({ blur: Number(($event.target as HTMLInputElement).value) })"
            />
            <span class="value-pill">{{ appearance.blur }}px</span>
            <span
              class="blur-dot"
              :style="{ filter: `blur(${Math.min(appearance.blur, 20)}px)` }"
              aria-hidden="true"
            ></span>
          </div>
        </div>
      </div>

      <!-- 存储位置 -->
      <div class="card section">
        <div class="section-head">
          <h3 class="section-title">存储位置</h3>
          <label class="migrate-check">
            <input type="checkbox" v-model="migrate" />
            迁移现有文件
          </label>
        </div>
        <p class="muted small section-note">
          数据和日志目录会在重启后生效；应用前可选择是否把现有文件迁移到新目录。
        </p>

        <div v-if="loadingStorage" class="storage-skeleton">
          <Skeleton width="100%" height="38px" radius="8px" />
          <Skeleton width="100%" height="38px" radius="8px" />
        </div>
        <template v-else>
          <div class="storage-row">
            <div class="storage-label">数据存储位置</div>
            <input class="input storage-input" v-model="dataDirInput" readonly />
            <button class="btn btn-ghost btn-sm" @click="chooseDataDir">选择文件夹</button>
            <button class="btn btn-ghost btn-sm" @click="resetDataDir">恢复默认</button>
          </div>
          <div class="storage-row">
            <div class="storage-label">日志文件位置</div>
            <input class="input storage-input" v-model="logDirInput" readonly />
            <button class="btn btn-ghost btn-sm" @click="chooseLogDir">选择文件夹</button>
            <button class="btn btn-ghost btn-sm" @click="resetLogDir">恢复默认</button>
          </div>

          <div class="storage-actions">
            <button class="btn btn-primary btn-sm" :disabled="applyingStorage" @click="applyStorage">
              <Spinner v-if="applyingStorage" :size="12" :width="2" />
              {{ applyingStorage ? "应用中" : "应用存储位置" }}
            </button>
            <button class="btn btn-ghost btn-sm" @click="openDataDir">打开数据目录</button>
          </div>
        </template>
      </div>

      <!-- 启动与关闭 -->
      <div class="card section">
        <h3 class="section-title">启动与关闭</h3>

        <div class="option-row">
          <div>
            <div class="option-label">开机自启动</div>
            <div class="muted small">Windows 登录系统后自动启动 NetTool</div>
          </div>
          <label class="switch">
            <input type="checkbox" :checked="autoStart" :disabled="autoStartBusy" @change="toggleAutostart" />
            <span class="slider-switch"></span>
          </label>
        </div>

        <div class="radio-list">
          <label
            v-for="opt in closeOptions"
            :key="opt.value"
            class="radio-item"
            :class="{ active: appSettings.closeBehavior === opt.value }"
          >
            <input
              type="radio"
              name="close-behavior"
              :value="opt.value"
              :checked="appSettings.closeBehavior === opt.value"
              @change="changeCloseBehavior(opt.value)"
            />
            <div class="radio-text">
              <div class="radio-title">{{ opt.label }}</div>
              <div class="muted small">{{ opt.desc }}</div>
            </div>
          </label>
        </div>
        <p class="muted small">后台运行时可通过托盘图标重新打开主界面，右键托盘菜单可退出程序。</p>
      </div>

      <!-- 运行环境 -->
      <div class="card section">
        <h3 class="section-title">运行环境</h3>
        <div class="env-grid">
          <div class="env-item">
            <span class="muted small">frpc 核心</span>
            <Skeleton v-if="loadingEnv" width="85%" height="11px" />
            <span v-else class="mono small">{{ core?.path || "未安装" }}</span>
          </div>
          <div class="env-item">
            <span class="muted small">核心版本</span>
            <Skeleton v-if="loadingEnv" width="60px" height="11px" />
            <span v-else class="small">{{ core?.version || "未安装" }}</span>
          </div>
          <div class="env-item">
            <span class="muted small">适用平台</span>
            <Skeleton v-if="loadingEnv" width="90px" height="11px" />
            <span v-else class="small">{{ core ? `${core.os} / ${core.arch}` : "-" }}</span>
          </div>
          <div class="env-item">
            <span class="muted small">应用标识</span>
            <span class="small mono">top.hn3.nettool</span>
          </div>
        </div>
        <div class="flex gap-2 mt-4">
          <button class="btn btn-ghost btn-sm" @click="openCoreDir">
            <Spinner v-if="loadingEnv" :size="12" :width="2" />
            打开核心目录
          </button>
        </div>
        <p class="muted small mt-2">
          服务器地址、认证令牌等连接参数请在「配置管理」中编辑。
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  animation: fadeUp 0.35s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.settings-grid {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-top: 20px;
  max-width: 920px;
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}
.section-title {
  font-size: 15px;
  font-weight: 600;
}
.section-note {
  margin: -4px 0 14px;
  line-height: 1.7;
}
.option-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 10px 0;
}
.option-row + .option-row {
  border-top: 1px dashed var(--border);
}
.option-row.disabled {
  opacity: 0.55;
}
.option-label {
  font-size: 13px;
  font-weight: 600;
}
.segmented {
  display: inline-flex;
  padding: 2px;
  border-radius: 999px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
}
.segmented button {
  padding: 5px 16px;
  border-radius: 999px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
}
.segmented button.active {
  background: var(--primary);
  color: #fff;
}
.hidden-input {
  display: none;
}
.bg-preview {
  margin: 6px 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--bg-elevated);
}
/* 预览区按 16:9 等比例缩放，完整展示整张图片 */
.bg-preview-stage {
  aspect-ratio: 16 / 9;
  max-height: 220px;
  display: grid;
  place-items: center;
  background: var(--bg);
  overflow: hidden;
}.bg-preview-stage img {
  display: block;
  width: 100%;
  height: 100%;
  /* 默认等比例裁切（cover），不再压缩变形 */
  object-fit: cover;
}
.bg-preview-stage.contain img {
  object-fit: contain;
}
.bg-preview-stage.stretch img {
  object-fit: fill;
}
.bg-preview-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  padding: 10px 12px;
  border-top: 1px solid var(--border);
}
.bg-mask-label {
  margin-left: 6px;
}
.bg-mask-range {
  flex: 1;
  min-width: 120px;
  accent-color: var(--primary);
}
.segmented.compact button {
  padding: 3px 10px;
  font-size: 12px;
}
.slider-control {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 220px;
}
.slider-control input[type="range"] {
  flex: 1;
  accent-color: var(--primary);
}
.value-pill {
  min-width: 46px;
  text-align: center;
  font-size: 12px;
  font-family: "JetBrains Mono", monospace;
  color: var(--text-secondary);
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 2px 8px;
}
.blur-dot {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  flex-shrink: 0;
  background: linear-gradient(135deg, var(--primary), var(--text-secondary));
}
.migrate-check {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
}
.migrate-check input {
  width: 14px;
  height: 14px;
  accent-color: var(--primary);
}
.storage-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}
.storage-label {
  width: 104px;
  flex-shrink: 0;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-secondary);
}
.storage-input {
  flex: 1;
  font-family: "JetBrains Mono", "Fira Code", monospace;
  font-size: 12px;
}
.storage-actions {
  display: flex;
  gap: 8px;
  margin-top: 6px;
}
.storage-skeleton {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.env-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
}
.env-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.mono {
  font-family: "JetBrains Mono", "Fira Code", monospace;
  word-break: break-all;
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
.slider-switch {
  position: absolute;
  inset: 0;
  background: var(--border);
  border-radius: 24px;
  transition: 0.25s;
}
.slider-switch::before {
  content: "";
  position: absolute;
  width: 18px;
  height: 18px;
  left: 3px;
  top: 3px;
  background: #fff;
  border-radius: 50%;
  transition: 0.25s;
}
.switch input:checked + .slider-switch {
  background: var(--primary);
}
.switch input:checked + .slider-switch::before {
  transform: translateX(18px);
}
.radio-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 8px 0 10px;
}
.radio-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  cursor: pointer;
  transition: all var(--transition);
}
.radio-item:hover {
  border-color: var(--border-light);
}
.radio-item.active {
  border-color: var(--primary);
  background: var(--primary-soft);
}
.radio-item input {
  margin-top: 3px;
  width: 15px;
  height: 15px;
  accent-color: var(--primary);
}
.radio-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.radio-title {
  font-size: 13px;
  font-weight: 600;
}
@keyframes fadeUp {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
