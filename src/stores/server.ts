import { defineStore } from "pinia";
import { ref, computed } from "vue";
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

  // 下一个 ID（临时，后续由后端生成）
  let nextId = 1;

  // 当前选中的 Server
  const activeServer = computed(() => {
    return servers.value.find((s) => s.server.id === activeServerId.value);
  });

  // 加载所有 Server（临时使用本地存储）
  const fetchServers = async () => {
    loading.value = true;
    try {
      // TODO: 后续调用 Tauri 命令
      // const data = await invoke<MqttServer[]>("get_servers");
      
      // 暂时从 localStorage 加载
      const stored = localStorage.getItem("mqtt-servers");
      if (stored) {
        const data = JSON.parse(stored) as MqttServer[];
        servers.value = data.map((server) => ({
          server,
          status: "disconnected" as ConnectionStatus,
        }));
        // 更新 nextId
        const maxId = Math.max(0, ...data.map((s) => s.id || 0));
        nextId = maxId + 1;
      }
    } finally {
      loading.value = false;
    }
  };

  // 保存到本地存储（临时）
  const saveToStorage = () => {
    const data = servers.value.map((s) => s.server);
    localStorage.setItem("mqtt-servers", JSON.stringify(data));
  };

  // 创建 Server
  const createServer = async (
    serverData: Omit<MqttServer, "id" | "created_at" | "updated_at">
  ): Promise<number> => {
    // TODO: 后续调用 Tauri 命令
    // const id = await invoke<number>("create_server", { server: serverData });
    
    const id = nextId++;
    const now = new Date().toISOString();
    const server: MqttServer = {
      ...serverData,
      id,
      created_at: now,
      updated_at: now,
    };

    servers.value.push({
      server,
      status: "disconnected",
    });

    saveToStorage();
    return id;
  };

  // 更新 Server
  const updateServer = async (serverData: MqttServer) => {
    // TODO: 后续调用 Tauri 命令
    // await invoke("update_server", { server: serverData });
    
    const index = servers.value.findIndex((s) => s.server.id === serverData.id);
    if (index !== -1) {
      servers.value[index].server = {
        ...serverData,
        updated_at: new Date().toISOString(),
      };
      saveToStorage();
    }
  };

  // 删除 Server
  const removeServer = async (id: number) => {
    // TODO: 后续调用 Tauri 命令
    // await invoke("delete_server", { id });
    
    const index = servers.value.findIndex((s) => s.server.id === id);
    if (index !== -1) {
      servers.value.splice(index, 1);
      if (activeServerId.value === id) {
        activeServerId.value = servers.value[0]?.server.id ?? null;
      }
      saveToStorage();
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
