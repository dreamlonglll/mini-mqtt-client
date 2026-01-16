import { createApp } from "vue";
import ElementPlus from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import en from "element-plus/es/locale/lang/en";
import App from "./App.vue";
import pinia from "./stores";
import i18n, { getActualLocale, type Locale } from "./i18n";
import { setupGlobalErrorHandler } from "./utils/errorHandler";

// Element Plus 暗黑主题
import "element-plus/theme-chalk/dark/css-vars.css";

// Element Plus 消息框样式（API调用的组件需要手动导入样式）
import "element-plus/theme-chalk/el-message-box.css";
import "element-plus/theme-chalk/el-message.css";
import "element-plus/theme-chalk/el-overlay.css";

// 全局样式
import "./assets/styles/index.scss";

// 设置全局错误处理器
setupGlobalErrorHandler();

// 获取存储的语言设置
const storedLocale = localStorage.getItem("mqtt-client-locale") as Locale | null;
const initialLocale = getActualLocale(storedLocale || "auto");

// 设置 i18n 语言
i18n.global.locale.value = initialLocale;

// 获取 Element Plus 语言配置
const getElementLocale = (locale: string) => {
  return locale === "zh-CN" ? zhCn : en;
};

const app = createApp(App);

app.use(pinia);
app.use(i18n);
app.use(ElementPlus, { locale: getElementLocale(initialLocale) });
app.mount("#app");

// 导出用于动态切换 Element Plus 语言的方法
export { getElementLocale };
