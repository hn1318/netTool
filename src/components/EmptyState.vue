<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string;
    description?: string;
    compact?: boolean;
  }>(),
  { title: "暂无数据", description: "", compact: false }
);
</script>

<template>
  <div class="empty-state" :class="{ compact }">
    <div class="empty-icon">
      <slot name="icon">
        <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.6">
          <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a4 4 0 0 0 0-8z" />
        </svg>
      </slot>
    </div>
    <p class="empty-title">{{ title }}</p>
    <p v-if="description" class="muted small empty-desc">{{ description }}</p>
    <div v-if="$slots.action" class="empty-action">
      <slot name="action" />
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 48px 20px;
  text-align: center;
}
.empty-state.compact {
  padding: 26px 16px;
}
.empty-icon {
  width: 52px;
  height: 52px;
  border-radius: 15px;
  background: var(--primary-soft);
  color: var(--primary);
  display: grid;
  place-items: center;
  margin-bottom: 6px;
}
.compact .empty-icon {
  width: 40px;
  height: 40px;
  border-radius: 12px;
}
.empty-title {
  font-size: 14px;
  font-weight: 600;
}
.empty-desc {
  max-width: 420px;
  line-height: 1.7;
}
.empty-action {
  margin-top: 10px;
}
</style>
