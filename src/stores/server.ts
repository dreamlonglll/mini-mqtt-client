import { defineStore } from "pinia";
import { ref, computed } from "vue";

// Server 连接状态
export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "error";

// MQTT Server 配置接口
export interface MqttServer {
  id: number;
  name: string;
  host: string;
  port: number;
  protocolVersion: "3.1.1" | "5.0";
  username?: string;
  password?: string;
  clientId?: string;
  keepAlive: number;
  cleanSession: boolean;
  useTls: boolean;
  caCert?: string;
  clientCert?: string;
  clientKey?: string;
  createdAt?: string;
  updatedAt?: string;
}

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

  // 当前选中的 Server
  const activeServer = computed(() => {
    return servers.value.find((s) => s.server.id === activeServerId.value);
  });

  // 添加 Server
  const addServer = (server: MqttServer) => {
    servers.value.push({
      server,
      status: "disconnected",
    });
  };

  // 更新 Server
  const updateServer = (id: number, data: Partial<MqttServer>) => {
    const index = servers.value.findIndex((s) => s.server.id === id);
    if (index !== -1) {
      servers.value[index].server = { ...servers.value[index].server, ...data };
    }
  };

  // 删除 Server
  const removeServer = (id: number) => {
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

  return {
    servers,
    activeServerId,
    activeServer,
    addServer,
    updateServer,
    removeServer,
    setActiveServer,
    setConnectionStatus,
  };
});
