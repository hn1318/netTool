<script setup lang="ts">
import { ref, computed } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue?: string;
    placeholder?: string;
    /** 默认是否以明文显示 */
    revealed?: boolean;
  }>(),
  { modelValue: "", placeholder: "", revealed: false }
);
const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
}>();

const show = ref(props.revealed);
const focused = ref(false);

const value = computed({
  get: () => props.modelValue ?? "",
  set: (v: string) => emit("update:modelValue", v),
});
</script>

<template>
  <div class="pw-field" :class="{ focused }">
    <input
      class="input pw-input"
      :type="show ? 'text' : 'password'"
      :value="value"
      :placeholder="placeholder"
      autocomplete="off"
      spellcheck="false"
      @input="value = ($event.target as HTMLInputElement).value"
      @focus="focused = true"
      @blur="focused = false"
    />
    <button
      type="button"
      class="eye"
      :title="show ? '隐藏' : '显示'"
      @click="show = !show"
    >
      <svg v-if="!show" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7-11-7-11-7z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
      <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
        <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
        <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24" />
        <path d="M1 1l22 22" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.pw-field {
  position: relative;
  display: flex;
  width: 100%;
}
.pw-input {
  padding-right: 38px;
  width: 100%;
  font-family: inherit;
}
.eye {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  transition: color var(--transition), background var(--transition);
}
.eye:hover {
  color: var(--text);
  background: var(--bg-hover);
}
.pw-field.focused .eye {
  color: var(--text-secondary);
}
</style>
