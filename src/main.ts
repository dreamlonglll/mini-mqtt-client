import { createApp } from "vue";
import App from "./App.vue";
import pinia from "./stores";

// Element Plus 暗黑主题
import "element-plus/theme-chalk/dark/css-vars.css";

// Element Plus 消息框样式（API调用的组件需要手动导入样式）
import "element-plus/theme-chalk/el-message-box.css";
import "element-plus/theme-chalk/el-message.css";
import "element-plus/theme-chalk/el-overlay.css";

// 全局样式
import "./assets/styles/index.scss";

const app = createApp(App);

app.use(pinia);
app.mount("#app");
