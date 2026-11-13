<script setup lang="ts">
withDefaults(
  defineProps<{
    width?: string;
    height?: string;
    radius?: string;
    /** 圆形骨架（头像等） */
    circle?: boolean;
  }>(),
  { width: "100%", height: "14px", radius: "6px", circle: false }
);
</script>

<template>
  <span
    class="skeleton"
    :class="{ circle }"
    aria-hidden="true"
    :style="{
      width,
      height: circle ? width : height,
      borderRadius: circle ? '50%' : radius,
    }"
  ></span>
</template>

<style scoped>
.skeleton {
  display: inline-block;
  background: linear-gradient(
    100deg,
    var(--bg-hover) 20%,
    var(--border) 40%,
    var(--bg-hover) 60%
  );
  background-size: 220% 100%;
  animation: skeleton-shimmer 1.25s ease-in-out infinite;
  vertical-align: middle;
}
@keyframes skeleton-shimmer {
  0% {
    background-position: 120% 0;
  }
  100% {
    background-position: -120% 0;
  }
}
</style>
