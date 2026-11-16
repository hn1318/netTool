import { ref } from "vue";

export interface Appearance {
  /** 圆角尺寸（px），0 = 直角 */
  radius: number;
  /** 是否启用磨砂玻璃 */
  glass: boolean;
  /** 模糊强度（px） */
  blur: number;
  /** 背景图片（dataURL，空表示不使用） */
  bgImage: string;
  /** 背景遮罩浓度 0~0.85，越大越暗 */
  bgMask: number;
  /** 背景图填充方式：fill 等比例铺满（裁切）/ contain 等比例完整显示 / stretch 拉伸 */
  bgFit: "fill" | "contain" | "stretch";
}

const KEY = "nettool_appearance";

export const DEFAULT_APPEARANCE: Appearance = {
  radius: 14,
  glass: true,
  blur: 18,
  bgImage: "",
  bgMask: 0.45,
  bgFit: "fill",
};

function clamp(n: number, min: number, max: number) {
  return Math.min(max, Math.max(min, n));
}

export function sanitize(a: Appearance): Appearance {
  return {
    radius: clamp(Math.round(Number(a.radius) || 0), 0, 24),
    glass: !!a.glass,
    blur: clamp(Math.round(Number(a.blur) || 0), 0, 60),
    bgImage: typeof a.bgImage === "string" ? a.bgImage : "",
    bgMask: clamp(Number(a.bgMask) || 0, 0, 0.85),
    bgFit: a.bgFit === "contain" || a.bgFit === "stretch" ? a.bgFit : "fill",
  };
}

export function loadAppearance(): Appearance {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULT_APPEARANCE };
    return sanitize({ ...DEFAULT_APPEARANCE, ...JSON.parse(raw) });
  } catch {
    return { ...DEFAULT_APPEARANCE };
  }
}

export function saveAppearance(a: Appearance) {
  try {
    localStorage.setItem(KEY, JSON.stringify(a));
  } catch {
    /* 忽略配额错误 */
  }
}

/** 当前生效的外观（响应式，组件可直接读取） */
export const appearance = ref<Appearance>(loadAppearance());

export function applyAppearance(input?: Appearance) {
  const a = sanitize(input ?? appearance.value);
  appearance.value = a;
  const root = document.documentElement;
  const blur = `${a.blur}px`;
  root.style.setProperty("--radius", `${a.radius}px`);
  root.style.setProperty("--radius-sm", `${Math.max(2, Math.round(a.radius * 0.6))}px`);
  root.style.setProperty("--radius-lg", `${Math.round(a.radius * 1.25)}px`);
  // 模糊强度直接写成样式字符串：Chromium 对自定义属性里的长度不支持 calc() 再计算
  root.style.setProperty("--glass-blur", blur);
  root.style.setProperty("--glass-filter", `blur(${blur}) saturate(165%)`);
  // 旧版本使用过 --glass-blur 变量，这里同步更新以保证向后兼容
  root.style.setProperty("--glass-blur", blur);  root.style.setProperty("--app-bg-mask", String(a.bgMask));
  root.dataset.bgFit = a.bgFit;
  root.classList.toggle("glass", a.glass);
  if (a.bgImage) {
    root.style.setProperty("--app-bg-image", `url("${a.bgImage}")`);
    root.classList.add("has-bg");
  } else {
    root.style.removeProperty("--app-bg-image");
    root.classList.remove("has-bg");
  }
}

export function updateAppearance(patch: Partial<Appearance>) {
  const next = sanitize({ ...appearance.value, ...patch });
  saveAppearance(next);
  applyAppearance(next);
  return next;
}

/** 读取图片并压缩为 dataURL，避免塞爆 localStorage */
export function imageToDataUrl(file: File, maxSize = 1920): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("读取图片失败"));
    reader.onload = () => {
      const img = new Image();
      img.onerror = () => reject(new Error("图片格式不受支持"));
      img.onload = () => {
        const scale = Math.min(1, maxSize / Math.max(img.width, img.height));
        const w = Math.max(1, Math.round(img.width * scale));
        const h = Math.max(1, Math.round(img.height * scale));
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d");
        if (!ctx) return reject(new Error("无法处理图片"));
        ctx.drawImage(img, 0, 0, w, h);
        resolve(canvas.toDataURL("image/jpeg", 0.86));
      };
      img.src = String(reader.result);
    };
    reader.readAsDataURL(file);
  });
}




