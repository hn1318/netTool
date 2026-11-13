<script setup lang="ts">
import { toasts, dismissToast, type ToastType } from "../composables/toast";

const icons: Record<ToastType, string> = {
  success:
    '<path d="m20 6-11 11-5-5" stroke-linecap="round" stroke-linejoin="round"/>',
  error:
    '<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16h.01" stroke-linecap="round"/>',
  warning:
    '<path d="M10.3 3.9 1.9 18a2 2 0 0 0 1.7 3h16.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01" stroke-linecap="round"/>',
  info:
    '<circle cx="12" cy="12" r="9"/><path d="M12 16v-4M12 8h.01" stroke-linecap="round"/>',
};
</script>

<template>
  <div class="toast-host" aria-live="polite" aria-atomic="false">
    <transition-group name="toast">
      <div v-for="t in toasts" :key="t.id" class="toast" :class="t.type" role="status">
        <span class="toast-icon">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" v-html="icons[t.type]"></svg>
        </span>
        <div class="toast-body">
          <div class="toast-title">{{ t.title }}</div>
          <div v-if="t.message" class="toast-message">{{ t.message }}</div>
        </div>
        <button class="toast-close" aria-label="关闭" @click="dismissToast(t.id)">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
        <span v-if="t.duration > 0" class="toast-progress" :style="{ animationDuration: `${t.duration}ms` }"></span>
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: 22px;
  bottom: 22px;
  z-index: 600;
  display: flex;
  flex-direction: column;
  gap: 10px;
  pointer-events: none;
  max-width: min(380px, calc(100vw - 44px));
}
.toast {
  pointer-events: auto;
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px 13px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--border);
  box-shadow: 0 10px 32px rgba(0, 0, 0, 0.28);
  color: var(--text);
  overflow: hidden;
}
:global(.glass) .toast {
  background: var(--glass-bg-strong);
  backdrop-filter: blur(var(--glass-blur));
  -webkit-backdrop-filter: blur(var(--glass-blur));
}
.toast-icon {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border-radius: 8px;
  display: grid;
  place-items: center;
  margin-top: 1px;
}
.toast.success .toast-icon {
  background: rgba(63, 191, 143, 0.14);
  color: var(--success);
}
.toast.error .toast-icon {
  background: rgba(240, 113, 120, 0.14);
  color: var(--danger);
}
.toast.warning .toast-icon {
  background: rgba(230, 175, 66, 0.16);
  color: var(--warning);
}
.toast.info .toast-icon {
  background: rgba(91, 141, 239, 0.14);
  color: var(--primary);
}
.toast-body {
  flex: 1;
  min-width: 0;
}
.toast-title {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
}
.toast-message {
  margin-top: 3px;
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.55;
  word-break: break-word;
}
.toast-close {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
}
.toast-close:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.toast-progress {
  position: absolute;
  left: 0;
  bottom: 0;
  height: 2px;
  width: 100%;
  transform-origin: left;
  animation: toast-progress linear forwards;
  background: var(--primary);
  opacity: 0.55;
}
.toast.success .toast-progress {
  background: var(--success);
}
.toast.error .toast-progress {
  background: var(--danger);
}
.toast.warning .toast-progress {
  background: var(--warning);
}
@keyframes toast-progress {
  from {
    transform: scaleX(1);
  }
  to {
    transform: scaleX(0);
  }
}
.toast-enter-active,
.toast-leave-active {
  transition: all 0.22s cubic-bezier(0.4, 0, 0.2, 1);
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(10px) scale(0.98);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(14px);
}
</style>
