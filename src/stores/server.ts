import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { MqttServer, ConnectionStatus } from "@/types/mqtt";

// 运行时 Server 状态
export interface ServerState {
  server: MqttServer;
  status: ConnectionStatus;
  errorMessage?: string;
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
      servers.value = data.map((server) => ({
        server,
        status: "disconnected" as ConnectionStatus,
      }));
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

    servers.value.unshift({
      server,
      status: "disconnected",
    });

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

  // 删除 Server
  const removeServer = async (id: number) => {
    await invoke("delete_server", { id });
    
    const index = servers.value.findIndex((s) => s.server.id === id);
    if (index !== -1) {
      servers.value.splice(index, 1);
      if (activeServerId.value === id) {
        activeServerId.value = servers.value[0]?.server.id ?? null;
      }
    }
  };

  // 设置当前 Server
  const setActiveServer = (id: number | null) => {
    activeServerId.value = id;
  };

  // 更新连接状态
  const setConnectionStatus = (
    id: number,
    status: ConnectionStatus,
    errorMessage?: string
  ) => {
    const serverState = servers.value.find((s) => s.server.id === id);
    if (serverState) {
      serverState.status = status;
      serverState.errorMessage = errorMessage;
    }
  };

  // 获取连接状态
  const getConnectionStatus = (id: number): ConnectionStatus => {
    const serverState = servers.value.find((s) => s.server.id === id);
    return serverState?.status || "disconnected";
  };

  // 复制 Server
  const duplicateServer = async (id: number) => {
    const source = servers.value.find((s) => s.server.id === id);
    if (source) {
      const newServer = {
        ...source.server,
        name: `${source.server.name} (副本)`,
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
    setConnectionStatus,
    getConnectionStatus,
    duplicateServer,
  };
});

// 导出类型
export type { ConnectionStatus };
