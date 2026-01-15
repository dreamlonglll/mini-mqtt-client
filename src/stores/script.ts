import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type ScriptType = "before_publish" | "after_receive";

export interface Script {
  id?: number;
  server_id: number;
  name: string;
  script_type: ScriptType;
  code: string;
  enabled: boolean;
  description?: string;
  created_at?: string;
  updated_at?: string;
}

export interface CreateScriptRequest {
  server_id: number;
  name: string;
  script_type: ScriptType;
  code: string;
  enabled: boolean;
  description?: string;
}

export interface UpdateScriptRequest {
  id: number;
  name?: string;
  code?: string;
  enabled?: boolean;
  description?: string;
}

export const useScriptStore = defineStore("script", () => {
  const scripts = ref<Script[]>([]);
  const loading = ref(false);

  // 加载服务器的所有脚本
  async function loadScripts(serverId: number) {
    loading.value = true;
    try {
      scripts.value = await invoke<Script[]>("list_scripts", { serverId });
    } catch (error) {
      console.error("加载脚本失败:", error);
      scripts.value = [];
    } finally {
      loading.value = false;
    }
  }

  // 获取启用的脚本（按类型）
  async function getEnabledScripts(serverId: number, scriptType: ScriptType): Promise<Script[]> {
    try {
      return await invoke<Script[]>("get_enabled_scripts", { serverId, scriptType });
    } catch (error) {
      console.error("获取启用脚本失败:", error);
      return [];
    }
  }

  // 创建脚本
  async function createScript(request: CreateScriptRequest): Promise<number> {
    const id = await invoke<number>("create_script", { request });
    await loadScripts(request.server_id);
    return id;
  }

  // 更新脚本
  async function updateScript(request: UpdateScriptRequest, serverId: number): Promise<void> {
    await invoke("update_script", { request });
    await loadScripts(serverId);
  }

  // 删除脚本
  async function deleteScript(id: number, serverId: number): Promise<void> {
    await invoke("delete_script", { id });
    await loadScripts(serverId);
  }

  // 切换脚本启用状态
  async function toggleScript(id: number, enabled: boolean, serverId: number): Promise<void> {
    await invoke("toggle_script", { id, enabled });
    await loadScripts(serverId);
  }

  return {
    scripts,
    loading,
    loadScripts,
    getEnabledScripts,
    createScript,
    updateScript,
    deleteScript,
    toggleScript,
  };
});
