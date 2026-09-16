/**
 * base64 / hex 编解码工具
 *
 * 全项目唯一的一份实现（MQTT store、脚本引擎沙箱、发布面板均委托此处），
 * hex 解码采用**严格**语义：去掉空白后长度为奇数或含非十六进制字符时抛出中文错误，
 * 不再静默产生 NaN 字节导致"发出去的内容与看到的不一致"。
 */

/** 全项目共用的 UTF-8 编解码器实例（热路径上不再逐条 new） */
export const utf8Decoder = new TextDecoder();
export const utf8Encoder = new TextEncoder();

/** 一次 String.fromCharCode 的实参分块大小（大数组直接展开会抛 RangeError） */
const BASE64_CHUNK_SIZE = 0x8000;

/** 合法 hex 字符集（允许空串） */
const HEX_PATTERN = /^[0-9A-Fa-f]*$/;

/**
 * Uint8Array 转 Base64
 *
 * 分块拼接，避免 `String.fromCharCode(...bytes)` 在大 payload 下超出参数上限。
 */
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += BASE64_CHUNK_SIZE) {
    binary += String.fromCharCode(...bytes.subarray(i, i + BASE64_CHUNK_SIZE));
  }
  return btoa(binary);
}

/** Base64 转 Uint8Array */
export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Uint8Array 转小写 hex 字符串（无分隔符） */
export function bytesToHex(bytes: Uint8Array): string {
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    hex += bytes[i].toString(16).padStart(2, "0");
  }
  return hex;
}

/**
 * hex 字符串转 Uint8Array（严格）
 *
 * 允许输入中含空白（会被忽略）；长度为奇数或含非法字符时抛出错误。
 */
export function hexToBytes(hex: string): Uint8Array {
  const cleaned = hex.replace(/\s/g, "");
  if (cleaned.length % 2 !== 0) {
    throw new Error("HEX 字符串长度必须为偶数");
  }
  if (!HEX_PATTERN.test(cleaned)) {
    throw new Error("HEX 字符串包含非法字符");
  }
  const bytes = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < cleaned.length; i += 2) {
    bytes[i / 2] = parseInt(cleaned.substring(i, i + 2), 16);
  }
  return bytes;
}

/**
 * 校验 hex 字符串是否可以被 `hexToBytes` 解码
 *
 * 供发布前置校验使用：字符集与偶数长度一并检查。
 */
export function isValidHex(hex: string): boolean {
  const cleaned = hex.replace(/\s/g, "");
  return cleaned.length % 2 === 0 && HEX_PATTERN.test(cleaned);
}
