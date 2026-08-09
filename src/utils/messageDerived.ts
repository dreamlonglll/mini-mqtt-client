import type { MqttMessage } from "@/types/mqtt";

export type PayloadDisplayFormat = "json" | "binary" | "text";

// 模块级复用的解码器（避免每条消息 new TextDecoder）
const sharedDecoder = new TextDecoder();

/**
 * 解码 payload 为文本（memo 到消息对象上，只算一次）
 */
export function getDecodedText(msg: MqttMessage): string {
  if (msg.decodedText === undefined) {
    msg.decodedText = msg.payload ? sharedDecoder.decode(msg.payload) : "";
  }
  return msg.decodedText;
}

/**
 * payload 的 HEX 字符串（惰性计算 + memo，避免搜索/渲染时反复生成 3 倍长度字符串）
 */
export function getHexText(msg: MqttMessage): string {
  if (msg.hexText === undefined) {
    const bytes = msg.payload;
    if (!bytes || bytes.length === 0) {
      msg.hexText = "";
    } else {
      const parts: string[] = new Array(bytes.length);
      for (let i = 0; i < bytes.length; i++) {
        parts[i] = bytes[i].toString(16).padStart(2, "0").toUpperCase();
      }
      msg.hexText = parts.join(" ");
    }
  }
  return msg.hexText;
}

/**
 * 自动检测 payload 展示格式
 */
function detectFormat(msg: MqttMessage): PayloadDisplayFormat {
  const bytes = msg.payload;
  if (!bytes || bytes.length === 0) return "text";

  const str = getDecodedText(msg);
  const trimmed = str.trim();

  // 尝试检测 JSON
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      JSON.parse(trimmed);
      return "json";
    } catch {
      // 不是有效的 JSON
    }
  }

  // 检测二进制数据（超过 10% 不可打印字符）
  let nonPrintableCount = 0;
  for (const byte of bytes) {
    if ((byte < 32 || byte > 126) && byte !== 9 && byte !== 10 && byte !== 13) {
      nonPrintableCount++;
    }
  }
  if (nonPrintableCount / bytes.length > 0.1) {
    return "binary";
  }

  return "text";
}

/**
 * 消息的展示格式（优先使用发送时指定的 payload_type，结果缓存到消息对象）
 */
export function getDisplayFormat(msg: MqttMessage): PayloadDisplayFormat {
  if (msg.payloadFormat === undefined) {
    if (msg.payload_type) {
      msg.payloadFormat =
        msg.payload_type === "hex" ? "binary" : msg.payload_type === "json" ? "json" : "text";
    } else {
      msg.payloadFormat = detectFormat(msg);
    }
  }
  return msg.payloadFormat;
}

/**
 * 入队时一次性计算派生数据，渲染/搜索热路径直接读缓存字段
 * （hexText 体积大，保持惰性，首次用到时才算）
 */
export function computeDerived(msg: MqttMessage): void {
  getDecodedText(msg);
  getDisplayFormat(msg);
}

// ===== 时间格式化（模块级复用 Intl.DateTimeFormat 实例，避免每次隐式新建） =====
const timeFormatters = new Map<string, Intl.DateTimeFormat>();
const fullFormatters = new Map<string, Intl.DateTimeFormat>();

function getTimeFormatter(locale: string): Intl.DateTimeFormat {
  let fmt = timeFormatters.get(locale);
  if (!fmt) {
    fmt = new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
    timeFormatters.set(locale, fmt);
  }
  return fmt;
}

function getFullFormatter(locale: string): Intl.DateTimeFormat {
  let fmt = fullFormatters.get(locale);
  if (!fmt) {
    fmt = new Intl.DateTimeFormat(locale, { dateStyle: "short", timeStyle: "medium" });
    fullFormatters.set(locale, fmt);
  }
  return fmt;
}

/**
 * 列表时间显示（HH:mm:ss.SSS）
 */
export function formatMsgTime(timestamp: number | undefined, locale: string): string {
  if (timestamp === undefined || timestamp === null) return "";
  const date = new Date(timestamp);
  const ms = date.getMilliseconds().toString().padStart(3, "0");
  return `${getTimeFormatter(locale).format(date)}.${ms}`;
}

/**
 * 详情完整时间显示
 */
export function formatMsgFullTime(timestamp: number | undefined, locale: string): string {
  if (timestamp === undefined || timestamp === null) return "";
  return getFullFormatter(locale).format(new Date(timestamp));
}
