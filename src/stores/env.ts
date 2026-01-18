import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest } from "@/types/mqtt";

export type { EnvVariable, CreateEnvVariableRequest, UpdateEnvVariableRequest };

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

  // 获取变量映射（用于替换）
  const variablesMap = computed(() => {
    const map: Record<string, string> = {};
    for (const v of variables.value) {
      map[v.name] = v.value;
    }
    return map;
  });

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
    return id;
  };

  // 更新环境变量
  const updateVariable = async (request: UpdateEnvVariableRequest) => {
    await invoke("update_env_variable", { request });
    // 更新本地列表
    const index = variables.value.findIndex((v) => v.id === request.id);
    if (index !== -1) {
      const current = variables.value[index];
      variables.value[index] = {
        ...current,
        name: request.name ?? current.name,
        value: request.value ?? current.value,
        description: request.description ?? current.description,
        updated_at: new Date().toISOString(),
      };
    }
  };

  // 删除环境变量
  const deleteVariable = async (id: number) => {
    await invoke("delete_env_variable", { id });
    // 从本地列表移除
    const index = variables.value.findIndex((v) => v.id === id);
    if (index !== -1) {
      variables.value.splice(index, 1);
    }
  };

  // 设置搜索关键词
  const setSearchKeyword = (keyword: string) => {
    searchKeyword.value = keyword;
  };

  // 替换文本中的环境变量
  const replaceVariables = (text: string): string => {
    if (!text) return text;
    return text.replace(/\{\{(\w+)\}\}/g, (match, varName) => {
      return variablesMap.value[varName] ?? match;
    });
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
    variablesMap,
    // 方法
    loadVariables,
    createVariable,
    updateVariable,
    deleteVariable,
    setSearchKeyword,
    replaceVariables,
    clearVariables,
  };
});
