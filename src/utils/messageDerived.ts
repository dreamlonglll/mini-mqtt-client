import type { MqttMessage } from "@/types/mqtt";
import { utf8Decoder } from "./encoding";

export type PayloadDisplayFormat = "json" | "binary" | "text";

/** 列表预览截断上限：超长文本一行展开会触发最贵的文本布局，完整内容在详情弹窗查看 */
export const PREVIEW_MAX_CHARS = 300;
export const PREVIEW_MAX_HEX_BYTES = 64;

// ===== 派生数据全部惰性计算并 memo 到消息对象上 =====
// 只有被渲染到可见行或被搜索命中的消息才会付出解码 / 格式检测的成本，
// 高频消息流里大多数消息在被裁掉之前从未被看过。

/**
 * 解码 payload 为文本（memo 到消息对象上，只算一次）
 */
export function getDecodedText(msg: MqttMessage): string {
  if (msg.decodedText === undefined) {
    msg.decodedText = msg.payload ? utf8Decoder.decode(msg.payload) : "";
  }
  return msg.decodedText;
}

/** 字节序列转大写 HEX，空格分隔 */
function hexOf(bytes: Uint8Array): string {
  if (bytes.length === 0) return "";
  const parts: string[] = new Array(bytes.length);
  for (let i = 0; i < bytes.length; i++) {
    parts[i] = bytes[i].toString(16).padStart(2, "0").toUpperCase();
  }
  return parts.join(" ");
}

/**
 * payload 的完整 HEX 字符串（3 倍长度，惰性 + memo；只在详情与二进制消息搜索时用到）
 */
export function getHexText(msg: MqttMessage): string {
  if (msg.hexText === undefined) {
    msg.hexText = msg.payload ? hexOf(msg.payload) : "";
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
 * 供关键词搜索用的小写文本：topic + 解码文本，二进制消息再附上 HEX
 *
 * memo 之后每次 flush 的过滤只剩 `includes`，不再对全部消息反复 toLowerCase；
 * HEX 只对二进制消息物化，避免搜一次就让所有消息常驻 3 倍长度的字符串。
 */
export function getSearchText(msg: MqttMessage): string {
  if (msg.searchText === undefined) {
    let text = `${msg.topic}\n${getDecodedText(msg)}`;
    if (getDisplayFormat(msg) === "binary") {
      text += `\n${getHexText(msg)}`;
    }
    msg.searchText = text.toLowerCase();
  }
  return msg.searchText;
}

/**
 * 列表行的预览文本（截断 + memo）：文本类取前 300 字符，二进制取前 64 字节的 HEX
 */
export function getPreviewText(msg: MqttMessage): string {
  if (msg.previewText === undefined) {
    if (getDisplayFormat(msg) === "binary") {
      const bytes = msg.payload ?? new Uint8Array();
      const hex = hexOf(bytes.subarray(0, PREVIEW_MAX_HEX_BYTES));
      msg.previewText = bytes.length > PREVIEW_MAX_HEX_BYTES ? `${hex} …` : hex;
    } else {
      const text = getDecodedText(msg);
      msg.previewText =
        text.length > PREVIEW_MAX_CHARS ? `${text.slice(0, PREVIEW_MAX_CHARS)} …` : text;
    }
  }
  return msg.previewText;
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
