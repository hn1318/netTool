import { ref } from "vue";

export type ToastType = "success" | "error" | "warning" | "info";

export interface ToastItem {
  id: number;
  type: ToastType;
  title: string;
  message?: string;
  /** 自动关闭毫秒数，0 表示不自动关闭 */
  duration: number;
}

export const toasts = ref<ToastItem[]>([]);
let seq = 1;
const MAX = 5;

function remove(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

export function dismissToast(id: number) {
  remove(id);
}

export function notify(
  type: ToastType,
  title: string,
  message?: string,
  duration = 4200
) {
  const item: ToastItem = { id: seq++, type, title, message, duration };
  toasts.value = [...toasts.value, item].slice(-MAX);
  if (duration > 0) {
    window.setTimeout(() => remove(item.id), duration);
  }
  return item.id;
}

export const toast = {
  success: (title: string, message?: string) => notify("success", title, message),
  error: (title: string, message?: string) => notify("error", title, message, 5600),
  warning: (title: string, message?: string) => notify("warning", title, message, 5000),
  info: (title: string, message?: string) => notify("info", title, message),
};
