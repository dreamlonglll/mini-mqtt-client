import { defineStore } from "pinia";
import { ref } from "vue";

export type Theme = "light" | "dark";
export type ViewType = "messages" | "templates";

// 复制到发布面板的消息数据
export interface CopyToPublishData {
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
  payloadType?: string; // "json" | "hex" | "text"
}

export const useAppStore = defineStore("app", () => {
  // 主题
  const theme = ref<Theme>("light");

  // 侧边栏折叠状态
  const sidebarCollapsed = ref(false);

  // 当前视图
  const currentView = ref<ViewType>("messages");

  // 复制到发布面板的消息
  const copyToPublishData = ref<CopyToPublishData | null>(null);

  // 切换主题
  const toggleTheme = () => {
    theme.value = theme.value === "light" ? "dark" : "light";
    applyTheme();
    saveTheme();
  };

  // 设置主题
  const setTheme = (newTheme: Theme) => {
    theme.value = newTheme;
    applyTheme();
    saveTheme();
  };

  // 应用主题到 DOM
  const applyTheme = () => {
    if (theme.value === "dark") {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  };

  // 保存主题到本地存储
  const saveTheme = () => {
    localStorage.setItem("mqtt-client-theme", theme.value);
  };

  // 初始化主题
  const initTheme = () => {
    const stored = localStorage.getItem("mqtt-client-theme");
    if (stored === "light" || stored === "dark") {
      theme.value = stored;
    } else if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
      theme.value = "dark";
    }
    applyTheme();
  };

  // 切换侧边栏
  const toggleSidebar = () => {
    sidebarCollapsed.value = !sidebarCollapsed.value;
  };

  // 设置当前视图
  const setCurrentView = (view: ViewType) => {
    currentView.value = view;
  };

  // 设置复制到发布面板的消息
  const setCopyToPublish = (data: CopyToPublishData) => {
    copyToPublishData.value = data;
  };

  // 清除复制到发布面板的消息
  const clearCopyToPublish = () => {
    copyToPublishData.value = null;
  };

  return {
    theme,
    sidebarCollapsed,
    currentView,
    copyToPublishData,
    toggleTheme,
    setTheme,
    initTheme,
    toggleSidebar,
    setCurrentView,
    setCopyToPublish,
    clearCopyToPublish,
  };
});
