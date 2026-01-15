import { defineStore } from "pinia";
import { ref } from "vue";
import { getCurrentWindow, type Theme as TauriTheme } from "@tauri-apps/api/window";

export type Theme = "light" | "dark" | "auto";
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
  
  // 主题监听取消函数
  let unlistenTheme: (() => void) | null = null;

  // 侧边栏折叠状态
  const sidebarCollapsed = ref(false);

  // 当前视图
  const currentView = ref<ViewType>("messages");

  // 复制到发布面板的消息
  const copyToPublishData = ref<CopyToPublishData | null>(null);

  // 获取系统主题（使用 Tauri API）
  const getSystemTheme = async (): Promise<"light" | "dark"> => {
    try {
      const appWindow = getCurrentWindow();
      const tauriTheme = await appWindow.theme();
      return tauriTheme === "dark" ? "dark" : "light";
    } catch {
      // 降级使用 CSS 媒体查询
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
  };

  // 系统主题变化处理
  const handleSystemThemeChange = (tauriTheme: TauriTheme | null) => {
    if (theme.value === "auto" && tauriTheme) {
      applyThemeToDOM(tauriTheme === "dark" ? "dark" : "light");
    }
  };

  // 切换主题
  const toggleTheme = () => {
    const themes: Theme[] = ["light", "dark", "auto"];
    const currentIndex = themes.indexOf(theme.value);
    theme.value = themes[(currentIndex + 1) % themes.length];
    applyTheme();
    saveTheme();
  };

  // 设置主题
  const setTheme = (newTheme: Theme) => {
    theme.value = newTheme;
    applyTheme();
    saveTheme();
  };

  // 应用主题到 DOM（内部方法）
  const applyThemeToDOM = (actualTheme: "light" | "dark") => {
    if (actualTheme === "dark") {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  };

  // 应用主题到 DOM
  const applyTheme = async () => {
    // 移除旧的监听器
    if (unlistenTheme) {
      unlistenTheme();
      unlistenTheme = null;
    }

    if (theme.value === "auto") {
      // 自动模式：根据系统主题设置，并监听变化
      const systemTheme = await getSystemTheme();
      applyThemeToDOM(systemTheme);
      
      // 监听系统主题变化
      try {
        const appWindow = getCurrentWindow();
        unlistenTheme = await appWindow.onThemeChanged(({ payload }) => {
          handleSystemThemeChange(payload);
        });
      } catch (e) {
        console.warn("无法监听系统主题变化:", e);
      }
    } else {
      // 手动模式：直接应用
      applyThemeToDOM(theme.value);
    }
  };

  // 保存主题到本地存储
  const saveTheme = () => {
    localStorage.setItem("mqtt-client-theme", theme.value);
  };

  // 初始化主题
  const initTheme = async () => {
    const stored = localStorage.getItem("mqtt-client-theme");
    if (stored === "light" || stored === "dark" || stored === "auto") {
      theme.value = stored;
    } else {
      // 默认使用自动模式
      theme.value = "auto";
    }
    await applyTheme();
  };

  // 清理监听器
  const cleanup = () => {
    if (unlistenTheme) {
      unlistenTheme();
      unlistenTheme = null;
    }
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
    cleanup,
  };
});
