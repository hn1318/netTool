<script setup lang="ts">
import { computed } from "vue";

export interface NavItem {
  id: string;
  label: string;
  icon: string;
}

const props = defineProps<{
  items: NavItem[];
  active: string;
  collapsed: boolean;
  theme: string;
}>();

const emit = defineEmits<{
  (e: "select", id: string): void;
  (e: "toggle"): void;
  (e: "toggle-theme"): void;
}>();

const navWidth = computed(() => (props.collapsed ? "72px" : "220px"));
</script>

<template>
  <aside class="sidebar" :style="{ width: navWidth }">
    <div class="logo-row" :class="{ collapsed }">
      <div class="logo-mark" :title="collapsed ? '点击切换亮暗主题' : ''" @click="collapsed && emit('toggle-theme')">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none">
          <path
            d="M12 2 4 6v6c0 5 3.4 8.7 8 10 4.6-1.3 8-5 8-10V6l-8-4z"
            fill="none"
            stroke="url(#lg)"
            stroke-width="1.8"
          />
          <path d="M12 7v5l3 2" stroke="url(#lg)" stroke-width="1.8" stroke-linecap="round" />
          <defs>
            <linearGradient id="lg" x1="0" y1="0" x2="24" y2="24">
              <stop stop-color="#4f8cff" />
              <stop offset="1" stop-color="#34d399" />
            </linearGradient>
          </defs>
        </svg>
      </div>
      <transition name="fade">
        <span v-if="!collapsed" class="logo-text">NetTool</span>
      </transition>
    </div>

    <nav class="nav-list">
      <button
        v-for="item in items"
        :key="item.id"
        class="nav-item"
        :class="{ active: active === item.id }"
        :title="collapsed ? item.label : ''"
        @click="emit('select', item.id)"
      >
        <span class="nav-icon" v-html="item.icon"></span>
        <transition name="fade">
          <span v-if="!collapsed" class="nav-label">{{ item.label }}</span>
        </transition>
        <span v-if="active === item.id" class="active-dot"></span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button class="collapse-btn" :title="collapsed ? '展开' : '折叠'" @click="emit('toggle')">
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          :style="{ transform: collapsed ? 'rotate(180deg)' : 'none', transition: 'transform .3s' }"
        >
          <path d="m15 18-6-6 6-6" />
        </svg>
      </button>
      <button
        v-if="!collapsed"
        class="collapse-btn theme-btn"
        :title="theme === 'dark' ? '切换为亮色' : '切换为暗色'"
        @click="emit('toggle-theme')"
      >
        <svg v-if="theme === 'dark'" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
        </svg>
        <svg v-else viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  height: 100%;
  background: var(--bg-elevated);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  flex-shrink: 0;
  overflow: hidden;
}

.logo-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 20px 16px;
  height: 68px;
  border-bottom: 1px solid var(--border);
}
.logo-row.collapsed {
  justify-content: center;
  padding: 20px 0;
}
.logo-mark {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.logo-row.collapsed .logo-mark {
  cursor: pointer;
}
.logo-text {
  font-size: 18px;
  font-weight: 700;
  letter-spacing: 0.5px;
  white-space: nowrap;
}

.nav-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 16px 12px;
  overflow-y: auto;
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 14px;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: 13.5px;
  font-weight: 500;
  transition: all var(--transition);
  text-align: left;
  width: 100%;
  overflow: hidden;
}
.nav-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.nav-item.active {
  background: var(--primary-soft);
  color: var(--primary);
}
.nav-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 20px;
}
.nav-icon :deep(svg) {
  width: 19px;
  height: 19px;
}
.nav-label {
  white-space: nowrap;
  opacity: 1;
}
.active-dot {
  position: absolute;
  right: 0;
  width: 3px;
  height: 20px;
  border-radius: 2px;
  background: var(--primary);
}

.sidebar-footer {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px 12px;
}
.collapse-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  padding: 10px;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  transition: all var(--transition);
}
.collapse-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.theme-btn:hover {
  color: var(--primary);
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
