import { createApp } from "vue";
import App from "./App.vue";
import pinia from "./stores";

// Element Plus 暗黑主题
import "element-plus/theme-chalk/dark/css-vars.css";

// 全局样式
import "./assets/styles/index.scss";

const app = createApp(App);

app.use(pinia);
app.mount("#app");
