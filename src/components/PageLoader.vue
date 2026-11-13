<script setup lang="ts">
import Spinner from "./Spinner.vue";
import Skeleton from "./Skeleton.vue";

withDefaults(
  defineProps<{
    title?: string;
    description?: string;
  }>(),
  { title: "加载中", description: "" }
);
</script>

<template>
  <div class="page-loader" role="status" aria-live="polite">
    <div class="loader-logo">
      <span class="pulse-ring"></span>
      <svg viewBox="0 0 24 24" width="30" height="30" fill="none">
        <path
          d="M12 2 4 6v6c0 5 3.4 8.7 8 10 4.6-1.3 8-5 8-10V6l-8-4z"
          stroke="url(#plg)"
          stroke-width="1.6"
        />
        <path d="M12 7v5l3 2" stroke="url(#plg)" stroke-width="1.6" stroke-linecap="round" />
        <defs>
          <linearGradient id="plg" x1="0" y1="0" x2="24" y2="24">
            <stop stop-color="#4f8cff" />
            <stop offset="1" stop-color="#34d399" />
          </linearGradient>
        </defs>
      </svg>
    </div>

    <div class="loader-text">
      <div class="loader-title">
        {{ title }}
        <Spinner :size="14" :width="2" />
      </div>
      <p v-if="description" class="muted small">{{ description }}</p>
    </div>

    <div class="loader-lines" aria-hidden="true">
      <Skeleton width="72%" height="10px" />
      <Skeleton width="52%" height="10px" />
      <Skeleton width="61%" height="10px" />
    </div>
  </div>
</template>

<style scoped>
.page-loader {
  min-height: 340px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 18px;
  padding: 48px 20px;
  text-align: center;
  animation: loader-in 0.35s ease;
}
.loader-logo {
  position: relative;
  width: 62px;
  height: 62px;
  display: grid;
  place-items: center;
  border-radius: 18px;
  background: var(--primary-soft);
  color: var(--primary);
}
.pulse-ring {
  position: absolute;
  inset: -6px;
  border-radius: 22px;
  border: 1px solid var(--primary);
  opacity: 0.35;
  animation: loader-pulse 1.8s ease-out infinite;
}
.loader-text {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.loader-title {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  font-size: 15px;
  font-weight: 600;
}
.loader-lines {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  width: 240px;
  max-width: 70vw;
}
@keyframes loader-pulse {
  0% {
    transform: scale(0.92);
    opacity: 0.45;
  }
  70% {
    transform: scale(1.12);
    opacity: 0;
  }
  100% {
    transform: scale(1.12);
    opacity: 0;
  }
}
@keyframes loader-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
