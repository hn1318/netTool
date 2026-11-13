<script setup lang="ts">
withDefaults(
  defineProps<{
    /** 尺寸（px） */
    size?: number;
    /** 线条宽度（px） */
    width?: number;
    /** 附加说明文字 */
    label?: string;
  }>(),
  { size: 18, width: 2, label: "" }
);
</script>

<template>
  <span class="spinner-wrap" role="status" :aria-label="label || '加载中'">
    <span
      class="spinner"
      :style="{ width: `${size}px`, height: `${size}px`, borderWidth: `${width}px` }"
    ></span>
    <span v-if="label" class="spinner-label">{{ label }}</span>
  </span>
</template>

<style scoped>
.spinner-wrap {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  vertical-align: middle;
}
.spinner {
  display: inline-block;
  border-style: solid;
  border-color: var(--border-light);
  border-top-color: var(--primary);
  border-radius: 50%;
  animation: spinner-rotate 0.8s linear infinite;
  flex-shrink: 0;
}
.spinner-label {
  font-size: 12px;
  color: var(--text-secondary);
}
@keyframes spinner-rotate {
  to {
    transform: rotate(360deg);
  }
}
</style>
