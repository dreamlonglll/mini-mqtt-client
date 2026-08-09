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
  client_key_password?: string;
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
 * MQTT 消息 (实时)
 */
export interface MqttMessage {
  id?: number;
  server_id: number;
  direction: "publish" | "receive";
  topic: string;
  payload?: Uint8Array;
  qos: 0 | 1 | 2;
  retain: boolean;
  /** Unix 毫秒时间戳 */
  timestamp?: number;
  /** 脚本处理错误信息 */
  scriptError?: string;
  /** 消息格式类型（发送时用户选择的格式） */
  payload_type?: "json" | "hex" | "text";
  /** 原始 payload 字节数（被截断时大于 payload.length） */
  originalLength?: number;
  /** payload 是否被后端截断 */
  truncated?: boolean;
  // ===== 入队时计算的派生数据缓存（渲染/搜索热路径直接读取） =====
  /** TextDecoder 解码后的文本 */
  decodedText?: string;
  /** HEX 字符串（惰性计算 + memo） */
  hexText?: string;
  /** 展示格式（payload_type 映射或自动检测结果） */
  payloadFormat?: "json" | "binary" | "text";
}

/**
 * 订阅类型
 */
export interface Subscription {
  id?: number;
  server_id: number;
  topic: string;
  qos: number;
  is_active: boolean;
  /** 订阅的颜色标记（用于消息列表中高亮显示） */
  color?: string;
  created_at?: string;
}

/**
 * 更新订阅请求
 */
export interface UpdateSubscriptionRequest {
  id: number;
  topic?: string;
  qos?: number;
  color?: string;
}

/**
 * 消息历史类型
 */
export interface MessageHistory {
  id?: number;
  server_id: number;
  topic: string;
  payload?: string;
  payload_format?: "text" | "json" | "hex";
  direction: "publish" | "receive";
  qos: number;
  retain: boolean;
  created_at?: string;
}

/**
 * 发布消息载荷
 */
export interface PublishPayload {
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
  format: "text" | "json" | "hex";
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
 * 环境变量
 */
export interface EnvVariable {
  id?: number;
  server_id: number;
  name: string;
  value: string;
  description?: string;
  created_at?: string;
  updated_at?: string;
}

/**
 * 创建环境变量请求
 */
export interface CreateEnvVariableRequest {
  server_id: number;
  name: string;
  value: string;
  description?: string;
}

/**
 * 更新环境变量请求
 */
export interface UpdateEnvVariableRequest {
  id: number;
  name?: string;
  value?: string;
  description?: string;
}

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
    client_key_password: "",
  };
}
