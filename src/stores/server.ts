import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MqttServer, ConnectionStatus } from "@/types/mqtt";
import { useMqttStore } from "@/stores/mqtt";
import { useSubscriptionStore } from "@/stores/subscription";
import { translate } from "@/i18n";

// 运行时 Server 状态
// 连接状态的唯一真源是 mqttStore.connectionStates，此处只保留配置本身
export interface ServerState {
  server: MqttServer;
}

export const useServerStore = defineStore("server", () => {
  // Server 列表
  const servers = ref<ServerState[]>([]);

  // 当前选中的 Server ID
  const activeServerId = ref<number | null>(null);

  // 加载状态
  const loading = ref(false);

  // 当前选中的 Server
  const activeServer = computed(() => {
    return servers.value.find((s) => s.server.id === activeServerId.value);
  });

  // 加载所有 Server
  const fetchServers = async () => {
    loading.value = true;
    try {
      const data = await invoke<MqttServer[]>("get_servers");
      servers.value = data.map((server) => ({ server }));
    } catch (e) {
      console.error("Failed to fetch servers:", e);
    } finally {
      loading.value = false;
    }
  };

  // 创建 Server
  const createServer = async (
    serverData: Omit<MqttServer, "id" | "created_at" | "updated_at">
  ): Promise<number> => {
    const id = await invoke<number>("create_server", { server: serverData });
    
    const now = new Date().toISOString();
    const server: MqttServer = {
      ...serverData,
      id,
      created_at: now,
      updated_at: now,
    };

    servers.value.unshift({ server });

    return id;
  };

  // 更新 Server
  const updateServer = async (serverData: MqttServer) => {
    await invoke("update_server", { server: serverData });
    
    const index = servers.value.findIndex((s) => s.server.id === serverData.id);
    if (index !== -1) {
      servers.value[index].server = {
        ...serverData,
        updated_at: new Date().toISOString(),
      };
    }
  };

  // 删除 Server（成功后级联清理其他 store 中该 Server 的残留状态）
  const removeServer = async (id: number) => {
    await invoke("delete_server", { id });

    const index = servers.value.findIndex((s) => s.server.id === id);
    if (index !== -1) {
      servers.value.splice(index, 1);
      if (activeServerId.value === id) {
        activeServerId.value = servers.value[0]?.server.id ?? null;
      }
    }

    // 级联清理：连接状态、消息、脚本 / 环境变量缓存、订阅缓存
    // （否则 ID 复用时新 Server 会读到旧 Server 的残留数据）
    useMqttStore().clearServerState(id);
    useSubscriptionStore().clearServerSubscriptions(id);
  };

  // 设置当前 Server
  const setActiveServer = (id: number | null) => {
    activeServerId.value = id;
  };

  // 复制 Server
  const duplicateServer = async (id: number) => {
    const source = servers.value.find((s) => s.server.id === id);
    if (source) {
      const newServer = {
        ...source.server,
        // 在调用时求值，跟随当前语言
        name: translate("server.duplicateSuffix", {
          name: source.server.name,
        }),
        client_id: "", // 清空 Client ID
      };
      // 移除 id 和时间戳
      const { id: _, created_at, updated_at, ...serverData } = newServer;
      await createServer(serverData);
    }
  };

  return {
    servers,
    activeServerId,
    activeServer,
    loading,
    fetchServers,
    createServer,
    updateServer,
    removeServer,
    setActiveServer,
    duplicateServer,
  };
});

// 导出类型
export type { ConnectionStatus };
