/**
 * MQTT Server 配置
 */
export interface MqttServer {
  id?: number;
  name: string;
  host: string;
  port: number;
  protocol_version: "3.1.1" | "5.0";
  username?: string;
  password?: string;
  client_id?: string;
  keep_alive: number;
  clean_session: boolean;
  use_tls: boolean;
  ca_cert?: string;
  client_cert?: string;
  client_key?: string;
  created_at?: string;
  updated_at?: string;
}

/**
 * 命令模板
 */
export interface CommandTemplate {
  id?: number;
  server_id: number;
  name: string;
  topic: string;
  payload?: string;
  qos: 0 | 1 | 2;
  retain: boolean;
  category?: string;
  created_at?: string;
  updated_at?: string;
}

/**
 * MQTT 消息
 */
export interface MqttMessage {
  id?: number;
  server_id: number;
  direction: "publish" | "receive";
  topic: string;
  payload?: Uint8Array;
  qos: 0 | 1 | 2;
  retain: boolean;
  timestamp?: string;
}

/**
 * 连接状态
 */
export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "error";

/**
 * 创建默认 Server 配置
 */
export function createDefaultServer(): MqttServer {
  return {
    name: "",
    host: "",
    port: 1883,
    protocol_version: "5.0",
    username: "",
    password: "",
    client_id: "",
    keep_alive: 60,
    clean_session: true,
    use_tls: false,
    ca_cert: "",
    client_cert: "",
    client_key: "",
  };
}
