import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";
import { applyAppearance, loadAppearance } from "./appearance";

// 在首帧渲染前应用外观设置，避免圆角 / 背景闪烁
applyAppearance(loadAppearance());

createApp(App).mount("#app");
