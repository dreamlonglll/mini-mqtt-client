import { createApp } from "vue";
import App from "./App.vue";
import pinia from "./stores";
import i18n, { getActualLocale, type Locale } from "./i18n";
import { setupGlobalErrorHandler } from "./utils/errorHandler";

// Element Plus 组件由 unplugin-vue-components 按需引入（见 vite.config.ts），
// 这里只补上全量样式里必需的部分：暗黑主题变量与 API 式组件的样式

// Element Plus 暗黑主题
import "element-plus/theme-chalk/dark/css-vars.css";

// API 式调用的组件不经过模板解析，样式需要手动导入
import "element-plus/theme-chalk/el-message-box.css";
import "element-plus/theme-chalk/el-message.css";
import "element-plus/theme-chalk/el-notification.css";
import "element-plus/theme-chalk/el-overlay.css";

// 虚拟滚动组件样式
import "vue-virtual-scroller/dist/vue-virtual-scroller.css";

// 全局样式
import "./assets/styles/index.scss";

// 设置全局错误处理器
setupGlobalErrorHandler();

// 获取存储的语言设置
const storedLocale = localStorage.getItem("mqtt-client-locale") as Locale | null;
const initialLocale = getActualLocale(storedLocale || "auto");

// 设置 i18n 语言
i18n.global.locale.value = initialLocale;

const app = createApp(App);

app.use(pinia);
app.use(i18n);
app.mount("#app");
