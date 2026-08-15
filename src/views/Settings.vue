<script setup lang="ts">
import { ref, watch } from "vue";
import type { AppSettings } from "../types";

const props = defineProps<{ settings: AppSettings }>();
const emit = defineEmits<{ (e: "update", s: AppSettings): void }>();

const form = ref<AppSettings>({ ...props.settings });

watch(
  () => props.settings,
  (v) => {
    form.value = { ...v };
  },
  { deep: true }
);

const saved = ref(false);
let saveTimer: number | undefined;

function save() {
  emit("update", { ...form.value });
  saved.value = true;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => (saved.value = false), 2000);
}
</script>

<template>
  <div class="settings">
    <div class="page-header flex justify-between items-center">
      <div>
        <h2>设置</h2>
        <p class="muted small">配置 FRP 常用参数</p>
      </div>
      <button class="btn btn-primary" @click="save">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
          <path d="M17 21v-8H7v8M7 3v5h8" />
        </svg>
        保存
      </button>
    </div>
    <transition name="fade">
      <div v-if="saved" class="save-toast">设置已保存</div>
    </transition>

    <div class="settings-grid">
      <!-- 服务器配置 -->
      <div class="card section">
        <h3 class="section-title">服务器连接</h3>
        <div class="form-grid">
          <div class="field">
            <label>服务器地址 (serverAddr)</label>
            <input class="input" v-model="form.serverAddr" />
          </div>
          <div class="field">
            <label>服务器端口 (serverPort)</label>
            <input class="input" type="number" v-model.number="form.serverPort" />
          </div>
          <div class="field">
            <label>传输协议 (transport)</label>
            <select class="select" v-model="form.transport">
              <option value="tcp">tcp</option>
              <option value="kcp">kcp</option>
              <option value="websocket">websocket</option>
            </select>
          </div>
          <div class="field">
            <label>认证令牌 (auth.token)</label>
            <input class="input" type="password" v-model="form.authToken" placeholder="可选" />
          </div>
          <div class="field">
            <label>用户 (user)</label>
            <input class="input" v-model="form.user" />
          </div>
          <div class="field">
            <label>TLS 加密</label>
            <label class="switch">
              <input type="checkbox" v-model="form.tlsEnable" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>

      <!-- 日志配置 -->
      <div class="card section">
        <h3 class="section-title">日志配置</h3>
        <div class="form-grid">
          <div class="field">
            <label>日志级别 (log.level)</label>
            <select class="select" v-model="form.logLevel">
              <option value="trace">trace</option>
              <option value="debug">debug</option>
              <option value="info">info</option>
              <option value="warn">warn</option>
              <option value="error">error</option>
            </select>
          </div>
          <div class="field">
            <label>日志文件 (log.to)</label>
            <input class="input" v-model="form.logFile" />
          </div>
          <div class="field">
            <label>日志保留天数 (log.maxDays)</label>
            <input class="input" type="number" v-model.number="form.logMaxDays" />
          </div>
        </div>
      </div>

      <!-- Web 控制台 -->
      <div class="card section">
        <h3 class="section-title">Web 控制台 (webServer)</h3>
        <div class="form-grid">
          <div class="field">
            <label>监听地址 (addr)</label>
            <input class="input" v-model="form.dashboardAddr" />
          </div>
          <div class="field">
            <label>端口 (port)</label>
            <input class="input" type="number" v-model.number="form.dashboardPort" />
          </div>
          <div class="field">
            <label>用户名 (user)</label>
            <input class="input" v-model="form.dashboardUser" />
          </div>
          <div class="field">
            <label>密码 (password)</label>
            <input class="input" type="password" v-model="form.dashboardPwd" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  animation: fadeUp 0.4s ease;
  position: relative;
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
  max-width: 860px;
}
.section-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}
.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
}
.save-toast {
  position: fixed;
  top: 20px;
  right: 24px;
  background: rgba(52, 211, 153, 0.15);
  color: var(--success);
  border: 1px solid rgba(52, 211, 153, 0.3);
  padding: 10px 18px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  z-index: 200;
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
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}
.fade-enter-from,
.fade-leave-to {
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
