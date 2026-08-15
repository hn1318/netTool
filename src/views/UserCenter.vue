<script setup lang="ts">
import { ref } from "vue";
import type { UserInfo } from "../types";

const props = defineProps<{ user: UserInfo; theme: string }>();
const emit = defineEmits<{
  (e: "update-user", u: UserInfo): void;
  (e: "set-theme", t: string): void;
}>();

const form = ref<UserInfo>({ ...props.user });

const lang = ref(localStorage.getItem("nettool_lang") || "zh-CN");
const autoStart = ref(localStorage.getItem("nettool_autostart") === "true");
const minimizeToTray = ref(localStorage.getItem("nettool_tray") === "true");

function saveProfile() {
  emit("update-user", { ...form.value });
}

function setTheme(t: string) {
  emit("set-theme", t);
}
function setLang(l: string) {
  lang.value = l;
  localStorage.setItem("nettool_lang", l);
}
function toggleAutoStart() {
  autoStart.value = !autoStart.value;
  localStorage.setItem("nettool_autostart", String(autoStart.value));
}
function toggleTray() {
  minimizeToTray.value = !minimizeToTray.value;
  localStorage.setItem("nettool_tray", String(minimizeToTray.value));
}
</script>

<template>
  <div class="user-center">
    <div class="page-header">
      <h2>用户中心</h2>
      <p class="muted small">账号信息与偏好设置</p>
    </div>

    <div class="profile-card card">
      <div class="avatar" :style="{ background: form.avatar || 'linear-gradient(135deg,#4f8cff,#34d399)' }">
        {{ form.nickname.slice(0, 1).toUpperCase() }}
      </div>
      <div class="profile-info">
        <div class="nickname">{{ form.nickname }}</div>
        <div class="muted small">{{ form.email }}</div>
      </div>
      <span class="badge" style="background: rgba(52,211,153,.15); color: #34d399">{{ form.plan }}</span>
    </div>

    <div class="columns">
      <!-- 账号信息 -->
      <div class="card">
        <h3 class="section-title">账号信息</h3>
        <div class="form-grid">
          <div class="field">
            <label>昵称</label>
            <input class="input" v-model="form.nickname" />
          </div>
          <div class="field">
            <label>邮箱</label>
            <input class="input" v-model="form.email" />
          </div>
          <div class="field">
            <label>订阅套餐</label>
            <input class="input" v-model="form.plan" disabled />
          </div>
          <div class="field">
            <label>到期时间</label>
            <input class="input" v-model="form.expireAt" disabled />
          </div>
        </div>
        <div class="mt-4">
          <button class="btn btn-primary" @click="saveProfile">保存资料</button>
        </div>
        <div class="usage mt-6">
          <div class="flex justify-between small">
            <span class="muted">带宽用量</span>
            <span>{{ form.bandwidth }}</span>
          </div>
          <div class="progress-bar mt-2">
            <div class="progress-fill" style="width: 62%"></div>
          </div>
        </div>
      </div>

      <!-- 偏好设置 -->
      <div class="card">
        <h3 class="section-title">偏好设置</h3>
        <div class="pref-list">
          <div class="pref-item">
            <div>
              <div class="pref-label">主题</div>
              <div class="muted small">选择界面主题</div>
            </div>
            <div class="flex gap-2">
              <button class="btn" :class="theme === 'dark' ? 'btn-primary' : 'btn-ghost'" @click="setTheme('dark')">
                深色
              </button>
              <button class="btn" :class="theme === 'light' ? 'btn-primary' : 'btn-ghost'" @click="setTheme('light')">
                浅色
              </button>
            </div>
          </div>
          <div class="pref-item">
            <div>
              <div class="pref-label">语言</div>
              <div class="muted small">界面显示语言</div>
            </div>
            <select class="select" style="width: 140px" @change="setLang(($event.target as HTMLSelectElement).value)">
              <option value="zh-CN" :selected="lang === 'zh-CN'">简体中文</option>
              <option value="en-US" :selected="lang === 'en-US'">English</option>
            </select>
          </div>
          <div class="pref-item">
            <div>
              <div class="pref-label">开机自启动</div>
              <div class="muted small">系统启动时自动运行</div>
            </div>
            <label class="switch">
              <input type="checkbox" :checked="autoStart" @change="toggleAutoStart" />
              <span class="slider"></span>
            </label>
          </div>
          <div class="pref-item">
            <div>
              <div class="pref-label">最小化到托盘</div>
              <div class="muted small">关闭窗口时最小化到系统托盘</div>
            </div>
            <label class="switch">
              <input type="checkbox" :checked="minimizeToTray" @change="toggleTray" />
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.user-center {
  animation: fadeUp 0.4s ease;
}
.page-header h2 {
  font-size: 20px;
  font-weight: 700;
}
.profile-card {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: 20px;
  margin-bottom: 16px;
}
.avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: 700;
  color: #fff;
  flex-shrink: 0;
}
.nickname {
  font-size: 18px;
  font-weight: 700;
}
.profile-info {
  flex: 1;
}
.columns {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: 16px;
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
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 14px;
}
.usage .progress-bar {
  height: 8px;
  background: var(--bg-elevated);
  border-radius: 4px;
  overflow: hidden;
}
.usage .progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--primary), var(--success));
  border-radius: 4px;
}
.pref-list {
  display: flex;
  flex-direction: column;
}
.pref-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 0;
  border-bottom: 1px solid var(--border);
}
.pref-item:last-child {
  border-bottom: none;
}
.pref-label {
  font-weight: 500;
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
