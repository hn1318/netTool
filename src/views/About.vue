<script setup lang="ts">
const appInfo = {
  name: "NetTool",
  version: "0.1.0",
  description: "跨平台 FRP 内网穿透连接管理工具",
};

const licenses = [
  { name: "Tauri", version: "2.x", license: "MIT / Apache-2.0" },
  { name: "Vue.js", version: "3.5", license: "MIT" },
  { name: "Vite", version: "8.x", license: "MIT" },
  { name: "ECharts", version: "5.x", license: "Apache-2.0" },
  { name: "FRP", version: "0.61", license: "Apache-2.0" },
  { name: "TypeScript", version: "6.x", license: "Apache-2.0" },
];

function openFrpSite() {
  window.open("https://github.com/fatedier/frp", "_blank");
}
function openTauriSite() {
  window.open("https://tauri.app", "_blank");
}
</script>

<template>
  <div class="about">
    <div class="hero card">
      <div class="logo">
        <svg viewBox="0 0 24 24" width="64" height="64" fill="none">
          <path d="M12 2 4 6v6c0 5 3.4 8.7 8 10 4.6-1.3 8-5 8-10V6l-8-4z" fill="none" stroke="url(#g)" stroke-width="1.5" />
          <path d="M12 7v5l3 2" stroke="url(#g)" stroke-width="1.5" stroke-linecap="round" />
          <defs>
            <linearGradient id="g" x1="0" y1="0" x2="24" y2="24">
              <stop stop-color="#4f8cff" />
              <stop offset="1" stop-color="#34d399" />
            </linearGradient>
          </defs>
        </svg>
      </div>
      <h1>{{ appInfo.name }}</h1>
      <div class="version">v{{ appInfo.version }}</div>
      <p class="muted">{{ appInfo.description }}</p>
    </div>

    <div class="card">
      <h3 class="section-title">开源许可</h3>
      <div class="license-list">
        <div v-for="l in licenses" :key="l.name" class="license-item">
          <div>
            <div class="license-name">{{ l.name }}</div>
            <div class="muted small">v{{ l.version }}</div>
          </div>
          <span class="badge" style="background: var(--bg-hover); color: var(--text-secondary)">{{ l.license }}</span>
        </div>
      </div>
    </div>

    <div class="links">
      <button class="btn btn-ghost" @click="openFrpSite">FRP 项目主页</button>
      <button class="btn btn-ghost" @click="openTauriSite">Tauri 官网</button>
    </div>

    <p class="copyright muted small">© 2026 NetTool. 保留所有权利。</p>
  </div>
</template>

<style scoped>
.about {
  animation: fadeUp 0.4s ease;
  max-width: 720px;
  margin: 0 auto;
  min-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  justify-content: center;
}
.hero {
  text-align: center;
  padding: 40px 20px;
  margin-bottom: 16px;
}
.logo {
  margin-bottom: 16px;
}
.hero h1 {
  font-size: 28px;
  font-weight: 700;
}
.version {
  color: var(--primary);
  font-weight: 600;
  margin: 4px 0 8px;
}
.section-title {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 12px;
}
.license-list {
  display: flex;
  flex-direction: column;
}
.license-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px solid var(--border);
}
.license-item:last-child {
  border-bottom: none;
}
.license-name {
  font-weight: 600;
}
.links {
  display: flex;
  gap: 12px;
  margin: 16px 0;
}
.copyright {
  text-align: center;
  margin-top: 24px;
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
