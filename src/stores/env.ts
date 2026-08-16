import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest } from "@/types/mqtt";
import { useMqttStore } from "@/stores/mqtt";

export type { EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest };

/**
 * 环境变量管理界面（EnvDrawer）的数据源。
 * 替换用的变量一律走 mqttStore.getCachedEnvVariables（按 serverId 隔离）。
 */
export const useEnvStore = defineStore("env", () => {
  // 状态
  const variables = ref<EnvVariable[]>([]);
  const loading = ref(false);
  const searchKeyword = ref("");

  // 过滤后的变量列表
  const filteredVariables = computed(() => {
    if (!searchKeyword.value) {
      return variables.value;
    }
    const keyword = searchKeyword.value.toLowerCase();
    return variables.value.filter(
      (v) =>
        v.name.toLowerCase().includes(keyword) ||
        (v.description && v.description.toLowerCase().includes(keyword))
    );
  });

  // 使某个变量所属 server 的替换缓存失效（找不到归属时全部失效）
  const invalidateCache = (serverId?: number) => {
    useMqttStore().clearEnvCache(serverId);
  };

  // 加载环境变量
  const loadVariables = async (serverId: number) => {
    loading.value = true;
    try {
      variables.value = await invoke<EnvVariable[]>("list_env_variables", {
        serverId,
      });
    } catch (error) {
      console.error("Failed to load env variables:", error);
      throw error;
    } finally {
      loading.value = false;
    }
  };

  // 创建环境变量
  const createVariable = async (request: CreateEnvVariableRequest): Promise<number> => {
    const id = await invoke<number>("create_env_variable", { request });
    // 添加到本地列表
    const now = new Date().toISOString();
    variables.value.push({
      id,
      server_id: request.server_id,
      name: request.name,
      value: request.value,
      description: request.description,
      created_at: now,
      updated_at: now,
    });
    invalidateCache(request.server_id);
    return id;
  };

  // 更新环境变量
  const updateVariable = async (request: UpdateEnvVariableRequest) => {
    await invoke("update_env_variable", { request });
    // 更新本地列表
    const index = variables.value.findIndex((v) => v.id === request.id);
    let serverId: number | undefined = undefined;
    if (index !== -1) {
      const current = variables.value[index];
      serverId = current.server_id;
      variables.value[index] = {
        ...current,
        name: request.name ?? current.name,
        value: request.value ?? current.value,
        description: request.description ?? current.description,
        updated_at: new Date().toISOString(),
      };
    }
    invalidateCache(serverId);
  };

  // 删除环境变量
  const deleteVariable = async (id: number) => {
    await invoke("delete_env_variable", { id });
    // 从本地列表移除
    const index = variables.value.findIndex((v) => v.id === id);
    const serverId = index !== -1 ? variables.value[index].server_id : undefined;
    if (index !== -1) {
      variables.value.splice(index, 1);
    }
    invalidateCache(serverId);
  };

  // 设置搜索关键词
  const setSearchKeyword = (keyword: string) => {
    searchKeyword.value = keyword;
  };

  // 清空状态
  const clearVariables = () => {
    variables.value = [];
    searchKeyword.value = "";
  };

  return {
    // 状态
    variables,
    loading,
    searchKeyword,
    filteredVariables,
    // 方法
    loadVariables,
    createVariable,
    updateVariable,
    deleteVariable,
    setSearchKeyword,
    clearVariables,
  };
});
