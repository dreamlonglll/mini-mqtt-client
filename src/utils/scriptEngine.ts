import type { Script } from "@/stores/script";
import { errorHandler, ErrorType } from "@/utils/errorHandler";

/**
 * 加密工具类 - 提供常用的加解密函数
 */
class CryptoUtils {
  /**
   * 字符串转 Uint8Array
   */
  static stringToBytes(str: string): Uint8Array {
    return new TextEncoder().encode(str);
  }

  /**
   * Uint8Array 转字符串
   */
  static bytesToString(bytes: Uint8Array): string {
    return new TextDecoder().decode(bytes);
  }

  /**
   * Uint8Array 转 Base64
   */
  static bytesToBase64(bytes: Uint8Array): string {
    return btoa(String.fromCharCode(...bytes));
  }

  /**
   * Base64 转 Uint8Array
   */
  static base64ToBytes(base64: string): Uint8Array {
    return Uint8Array.from(atob(base64), c => c.charCodeAt(0));
  }

  /**
   * Uint8Array 转 Hex
   */
  static bytesToHex(bytes: Uint8Array): string {
    return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  /**
   * Hex 转 Uint8Array
   */
  static hexToBytes(hex: string): Uint8Array {
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < hex.length; i += 2) {
      bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
    }
    return bytes;
  }

  /**
   * 生成随机字节
   */
  static randomBytes(length: number): Uint8Array {
    const bytes = new Uint8Array(length);
    crypto.getRandomValues(bytes);
    return bytes;
  }

  /**
   * 生成随机密钥（Base64）
   */
  static generateKey(bits: number = 128): string {
    return this.bytesToBase64(this.randomBytes(bits / 8));
  }

  /**
   * 生成随机 IV（Base64，默认12字节用于 AES-GCM）
   */
  static generateIv(length: number = 12): string {
    return this.bytesToBase64(this.randomBytes(length));
  }

  /**
   * SHA-256 哈希
   */
  static async sha256(data: string): Promise<string> {
    const bytes = this.stringToBytes(data);
    const hash = await crypto.subtle.digest('SHA-256', bytes);
    return this.bytesToHex(new Uint8Array(hash));
  }

  /**
   * SHA-1 哈希
   */
  static async sha1(data: string): Promise<string> {
    const bytes = this.stringToBytes(data);
    const hash = await crypto.subtle.digest('SHA-1', bytes);
    return this.bytesToHex(new Uint8Array(hash));
  }

  /**
   * MD5 哈希（纯 JS 实现）
   */
  static md5(data: string): string {
    // 简化的 MD5 实现
    function md5cycle(x: number[], k: number[]) {
      let a = x[0], b = x[1], c = x[2], d = x[3];
      a = ff(a, b, c, d, k[0], 7, -680876936);
      d = ff(d, a, b, c, k[1], 12, -389564586);
      c = ff(c, d, a, b, k[2], 17, 606105819);
      b = ff(b, c, d, a, k[3], 22, -1044525330);
      a = ff(a, b, c, d, k[4], 7, -176418897);
      d = ff(d, a, b, c, k[5], 12, 1200080426);
      c = ff(c, d, a, b, k[6], 17, -1473231341);
      b = ff(b, c, d, a, k[7], 22, -45705983);
      a = ff(a, b, c, d, k[8], 7, 1770035416);
      d = ff(d, a, b, c, k[9], 12, -1958414417);
      c = ff(c, d, a, b, k[10], 17, -42063);
      b = ff(b, c, d, a, k[11], 22, -1990404162);
      a = ff(a, b, c, d, k[12], 7, 1804603682);
      d = ff(d, a, b, c, k[13], 12, -40341101);
      c = ff(c, d, a, b, k[14], 17, -1502002290);
      b = ff(b, c, d, a, k[15], 22, 1236535329);
      a = gg(a, b, c, d, k[1], 5, -165796510);
      d = gg(d, a, b, c, k[6], 9, -1069501632);
      c = gg(c, d, a, b, k[11], 14, 643717713);
      b = gg(b, c, d, a, k[0], 20, -373897302);
      a = gg(a, b, c, d, k[5], 5, -701558691);
      d = gg(d, a, b, c, k[10], 9, 38016083);
      c = gg(c, d, a, b, k[15], 14, -660478335);
      b = gg(b, c, d, a, k[4], 20, -405537848);
      a = gg(a, b, c, d, k[9], 5, 568446438);
      d = gg(d, a, b, c, k[14], 9, -1019803690);
      c = gg(c, d, a, b, k[3], 14, -187363961);
      b = gg(b, c, d, a, k[8], 20, 1163531501);
      a = gg(a, b, c, d, k[13], 5, -1444681467);
      d = gg(d, a, b, c, k[2], 9, -51403784);
      c = gg(c, d, a, b, k[7], 14, 1735328473);
      b = gg(b, c, d, a, k[12], 20, -1926607734);
      a = hh(a, b, c, d, k[5], 4, -378558);
      d = hh(d, a, b, c, k[8], 11, -2022574463);
      c = hh(c, d, a, b, k[11], 16, 1839030562);
      b = hh(b, c, d, a, k[14], 23, -35309556);
      a = hh(a, b, c, d, k[1], 4, -1530992060);
      d = hh(d, a, b, c, k[4], 11, 1272893353);
      c = hh(c, d, a, b, k[7], 16, -155497632);
      b = hh(b, c, d, a, k[10], 23, -1094730640);
      a = hh(a, b, c, d, k[13], 4, 681279174);
      d = hh(d, a, b, c, k[0], 11, -358537222);
      c = hh(c, d, a, b, k[3], 16, -722521979);
      b = hh(b, c, d, a, k[6], 23, 76029189);
      a = hh(a, b, c, d, k[9], 4, -640364487);
      d = hh(d, a, b, c, k[12], 11, -421815835);
      c = hh(c, d, a, b, k[15], 16, 530742520);
      b = hh(b, c, d, a, k[2], 23, -995338651);
      a = ii(a, b, c, d, k[0], 6, -198630844);
      d = ii(d, a, b, c, k[7], 10, 1126891415);
      c = ii(c, d, a, b, k[14], 15, -1416354905);
      b = ii(b, c, d, a, k[5], 21, -57434055);
      a = ii(a, b, c, d, k[12], 6, 1700485571);
      d = ii(d, a, b, c, k[3], 10, -1894986606);
      c = ii(c, d, a, b, k[10], 15, -1051523);
      b = ii(b, c, d, a, k[1], 21, -2054922799);
      a = ii(a, b, c, d, k[8], 6, 1873313359);
      d = ii(d, a, b, c, k[15], 10, -30611744);
      c = ii(c, d, a, b, k[6], 15, -1560198380);
      b = ii(b, c, d, a, k[13], 21, 1309151649);
      a = ii(a, b, c, d, k[4], 6, -145523070);
      d = ii(d, a, b, c, k[11], 10, -1120210379);
      c = ii(c, d, a, b, k[2], 15, 718787259);
      b = ii(b, c, d, a, k[9], 21, -343485551);
      x[0] = add32(a, x[0]);
      x[1] = add32(b, x[1]);
      x[2] = add32(c, x[2]);
      x[3] = add32(d, x[3]);
    }

    function cmn(q: number, a: number, b: number, x: number, s: number, t: number) {
      a = add32(add32(a, q), add32(x, t));
      return add32((a << s) | (a >>> (32 - s)), b);
    }

    function ff(a: number, b: number, c: number, d: number, x: number, s: number, t: number) {
      return cmn((b & c) | ((~b) & d), a, b, x, s, t);
    }

    function gg(a: number, b: number, c: number, d: number, x: number, s: number, t: number) {
      return cmn((b & d) | (c & (~d)), a, b, x, s, t);
    }

    function hh(a: number, b: number, c: number, d: number, x: number, s: number, t: number) {
      return cmn(b ^ c ^ d, a, b, x, s, t);
    }

    function ii(a: number, b: number, c: number, d: number, x: number, s: number, t: number) {
      return cmn(c ^ (b | (~d)), a, b, x, s, t);
    }

    function add32(a: number, b: number) {
      return (a + b) & 0xFFFFFFFF;
    }

    function md5blk(s: string) {
      const md5blks: number[] = [];
      for (let i = 0; i < 64; i += 4) {
        md5blks[i >> 2] = s.charCodeAt(i) + (s.charCodeAt(i + 1) << 8) + (s.charCodeAt(i + 2) << 16) + (s.charCodeAt(i + 3) << 24);
      }
      return md5blks;
    }

    let n = data.length;
    let state = [1732584193, -271733879, -1732584194, 271733878];
    let i: number;

    for (i = 64; i <= n; i += 64) {
      md5cycle(state, md5blk(data.substring(i - 64, i)));
    }

    data = data.substring(i - 64);
    const tail = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    for (i = 0; i < data.length; i++) {
      tail[i >> 2] |= data.charCodeAt(i) << ((i % 4) << 3);
    }
    tail[i >> 2] |= 0x80 << ((i % 4) << 3);
    if (i > 55) {
      md5cycle(state, tail);
      for (i = 0; i < 16; i++) tail[i] = 0;
    }
    tail[14] = n * 8;
    md5cycle(state, tail);

    const hex = '0123456789abcdef';
    let result = '';
    for (i = 0; i < 4; i++) {
      for (let j = 0; j < 4; j++) {
        result += hex.charAt((state[i] >> (j * 8 + 4)) & 0x0F) + hex.charAt((state[i] >> (j * 8)) & 0x0F);
      }
    }
    return result;
  }

  /**
   * HMAC-SHA256
   */
  static async hmacSha256(key: string, data: string): Promise<string> {
    const keyBytes = this.stringToBytes(key);
    const dataBytes = this.stringToBytes(data);
    
    const cryptoKey = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']
    );
    
    const signature = await crypto.subtle.sign('HMAC', cryptoKey, dataBytes);
    return this.bytesToHex(new Uint8Array(signature));
  }

  /**
   * AES-GCM 加密
   * @param plaintext 明文
   * @param keyBase64 密钥（Base64格式，128/192/256位）
   * @param ivBase64 可选IV（Base64格式，12字节），不提供则自动生成
   * @returns Base64格式的密文（包含IV）
   */
  static async aesGcmEncrypt(plaintext: string, keyBase64: string, ivBase64?: string): Promise<string> {
    const keyBytes = this.base64ToBytes(keyBase64);
    const iv = ivBase64 ? this.base64ToBytes(ivBase64) : this.randomBytes(12);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-GCM' }, false, ['encrypt']
    );
    
    const encoded = this.stringToBytes(plaintext);
    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv }, key, encoded
    );
    
    // 返回 iv + ciphertext 的 base64
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return this.bytesToBase64(combined);
  }

  /**
   * AES-GCM 解密
   * @param ciphertextBase64 密文（Base64格式，包含IV）
   * @param keyBase64 密钥（Base64格式）
   * @returns 解密后的明文
   */
  static async aesGcmDecrypt(ciphertextBase64: string, keyBase64: string): Promise<string> {
    const keyBytes = this.base64ToBytes(keyBase64);
    const combined = this.base64ToBytes(ciphertextBase64);
    
    const iv = combined.slice(0, 12);
    const ciphertext = combined.slice(12);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-GCM' }, false, ['decrypt']
    );
    
    const decrypted = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv }, key, ciphertext
    );
    
    return this.bytesToString(new Uint8Array(decrypted));
  }

  /**
   * AES-CBC 加密
   * @param plaintext 明文
   * @param keyBase64 密钥（Base64格式，128/192/256位）
   * @param ivBase64 可选IV（Base64格式，16字节），不提供则自动生成
   * @returns Base64格式的密文（包含IV）
   */
  static async aesCbcEncrypt(plaintext: string, keyBase64: string, ivBase64?: string): Promise<string> {
    const keyBytes = this.base64ToBytes(keyBase64);
    const iv = ivBase64 ? this.base64ToBytes(ivBase64) : this.randomBytes(16);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-CBC' }, false, ['encrypt']
    );
    
    // PKCS7 填充
    const encoded = this.stringToBytes(plaintext);
    const blockSize = 16;
    const padLen = blockSize - (encoded.length % blockSize);
    const padded = new Uint8Array(encoded.length + padLen);
    padded.set(encoded);
    padded.fill(padLen, encoded.length);
    
    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-CBC', iv }, key, padded
    );
    
    // 返回 iv + ciphertext 的 base64
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return this.bytesToBase64(combined);
  }

  /**
   * AES-CBC 解密
   * @param ciphertextBase64 密文（Base64格式，包含IV）
   * @param keyBase64 密钥（Base64格式）
   * @returns 解密后的明文
   */
  static async aesCbcDecrypt(ciphertextBase64: string, keyBase64: string): Promise<string> {
    const keyBytes = this.base64ToBytes(keyBase64);
    const combined = this.base64ToBytes(ciphertextBase64);
    
    const iv = combined.slice(0, 16);
    const ciphertext = combined.slice(16);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-CBC' }, false, ['decrypt']
    );
    
    const decrypted = await crypto.subtle.decrypt(
      { name: 'AES-CBC', iv }, key, ciphertext
    );
    
    // 移除 PKCS7 填充
    const decryptedBytes = new Uint8Array(decrypted);
    const padLen = decryptedBytes[decryptedBytes.length - 1];
    return this.bytesToString(decryptedBytes.slice(0, -padLen));
  }

  /**
   * AES-GCM 加密（Hex 格式密钥和 IV）
   * @param plaintext 明文
   * @param keyHex 密钥（Hex格式，128/256位，即32或64个十六进制字符）
   * @param ivHex 可选IV（Hex格式，12字节即24个十六进制字符），不提供则自动生成
   * @returns Hex格式的密文（包含IV）
   */
  static async aesGcmEncryptHex(plaintext: string, keyHex: string, ivHex?: string): Promise<string> {
    const keyBytes = this.hexToBytes(keyHex);
    const iv = ivHex ? this.hexToBytes(ivHex) : this.randomBytes(12);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-GCM' }, false, ['encrypt']
    );
    
    const encoded = this.stringToBytes(plaintext);
    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv }, key, encoded
    );
    
    // 返回 iv + ciphertext 的 hex
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return this.bytesToHex(combined);
  }

  /**
   * AES-GCM 解密（Hex 格式密钥）
   * @param ciphertextHex 密文（Hex格式，包含IV）
   * @param keyHex 密钥（Hex格式）
   * @returns 解密后的明文
   */
  static async aesGcmDecryptHex(ciphertextHex: string, keyHex: string): Promise<string> {
    const keyBytes = this.hexToBytes(keyHex);
    const combined = this.hexToBytes(ciphertextHex);
    
    const iv = combined.slice(0, 12);
    const ciphertext = combined.slice(12);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-GCM' }, false, ['decrypt']
    );
    
    const decrypted = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv }, key, ciphertext
    );
    
    return this.bytesToString(new Uint8Array(decrypted));
  }

  /**
   * AES-CBC 加密（Hex 格式密钥和 IV）
   * @param plaintext 明文
   * @param keyHex 密钥（Hex格式，128/256位）
   * @param ivHex 可选IV（Hex格式，16字节即32个十六进制字符），不提供则自动生成
   * @returns Hex格式的密文（包含IV）
   */
  static async aesCbcEncryptHex(plaintext: string, keyHex: string, ivHex?: string): Promise<string> {
    const keyBytes = this.hexToBytes(keyHex);
    const iv = ivHex ? this.hexToBytes(ivHex) : this.randomBytes(16);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-CBC' }, false, ['encrypt']
    );
    
    // PKCS7 填充
    const encoded = this.stringToBytes(plaintext);
    const blockSize = 16;
    const padLen = blockSize - (encoded.length % blockSize);
    const padded = new Uint8Array(encoded.length + padLen);
    padded.set(encoded);
    padded.fill(padLen, encoded.length);
    
    const ciphertext = await crypto.subtle.encrypt(
      { name: 'AES-CBC', iv }, key, padded
    );
    
    // 返回 iv + ciphertext 的 hex
    const combined = new Uint8Array(iv.length + ciphertext.byteLength);
    combined.set(iv);
    combined.set(new Uint8Array(ciphertext), iv.length);
    return this.bytesToHex(combined);
  }

  /**
   * AES-CBC 解密（Hex 格式密钥）
   * @param ciphertextHex 密文（Hex格式，包含IV）
   * @param keyHex 密钥（Hex格式）
   * @returns 解密后的明文
   */
  static async aesCbcDecryptHex(ciphertextHex: string, keyHex: string): Promise<string> {
    const keyBytes = this.hexToBytes(keyHex);
    const combined = this.hexToBytes(ciphertextHex);
    
    const iv = combined.slice(0, 16);
    const ciphertext = combined.slice(16);
    
    const key = await crypto.subtle.importKey(
      'raw', keyBytes, { name: 'AES-CBC' }, false, ['decrypt']
    );
    
    const decrypted = await crypto.subtle.decrypt(
      { name: 'AES-CBC', iv }, key, ciphertext
    );
    
    // 移除 PKCS7 填充
    const decryptedBytes = new Uint8Array(decrypted);
    const padLen = decryptedBytes[decryptedBytes.length - 1];
    return this.bytesToString(decryptedBytes.slice(0, -padLen));
  }

  /**
   * XOR 加解密（对称）
   */
  static xor(data: string, key: string): string {
    let result = '';
    for (let i = 0; i < data.length; i++) {
      result += String.fromCharCode(data.charCodeAt(i) ^ key.charCodeAt(i % key.length));
    }
    return result;
  }

  /**
   * CRC32 校验
   */
  static crc32(data: string): string {
    let crc = 0xFFFFFFFF;
    const table: number[] = [];
    
    // 生成 CRC 表
    for (let i = 0; i < 256; i++) {
      let c = i;
      for (let j = 0; j < 8; j++) {
        c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
      }
      table[i] = c;
    }
    
    // 计算 CRC
    for (let i = 0; i < data.length; i++) {
      crc = table[(crc ^ data.charCodeAt(i)) & 0xFF] ^ (crc >>> 8);
    }
    
    return ((crc ^ 0xFFFFFFFF) >>> 0).toString(16).padStart(8, '0');
  }
}

/**
 * 脚本执行引擎
 * 安全地执行用户定义的 JavaScript 脚本
 */
export class ScriptEngine {
  /**
   * 执行发送前处理脚本
   * @param scripts 要执行的脚本列表
   * @param payload 原始 payload
   * @param envVariables 环境变量映射（可选）
   * @returns 处理后的 payload
   */
  static async executeBeforePublish(
    scripts: Script[], 
    payload: string, 
    envVariables?: Record<string, string>
  ): Promise<string> {
    let result = payload;
    
    for (const script of scripts) {
      try {
        result = await this.executeScript(script.code, { payload: result, env: envVariables });
      } catch (error: any) {
        const errorMessage = `脚本执行失败 [${script.name}]: ${error?.message || error}`;
        console.error(errorMessage, error);
        // 写入错误日志（静默处理，不显示通知）
        errorHandler.handle(errorMessage, ErrorType.SCRIPT, true);
        // 抛出错误，让调用方决定如何处理
        throw error;
      }
    }
    
    return result;
  }

  /**
   * 执行接收后处理脚本
   * @param scripts 要执行的脚本列表
   * @param payload 原始 payload
   * @param topic 消息主题
   * @param envVariables 环境变量映射（可选）
   * @returns 处理后的 payload
   */
  static async executeAfterReceive(
    scripts: Script[], 
    payload: string, 
    topic: string,
    envVariables?: Record<string, string>
  ): Promise<string> {
    let result = payload;
    
    for (const script of scripts) {
      try {
        result = await this.executeScript(script.code, { payload: result, topic, env: envVariables });
      } catch (error: any) {
        const errorMessage = `脚本执行失败 [${script.name}]: ${error?.message || error}`;
        console.error(errorMessage, error);
        // 写入错误日志（静默处理，不显示通知）
        errorHandler.handle(errorMessage, ErrorType.SCRIPT, true);
        // 抛出错误，让调用方可以在消息列表中展示
        throw error;
      }
    }
    
    return result;
  }

  /**
   * 执行单个脚本
   * @param code 脚本代码
   * @param context 执行上下文
   * @returns 脚本返回值
   */
  private static async executeScript(
    code: string,
    context: { payload: string; topic?: string; env?: Record<string, string> }
  ): Promise<string> {
    // 创建环境变量对象，支持直接属性访问和方法调用
    const envData = context.env || {};
    const envObject = {
      // 直接访问变量值：env.VAR_NAME
      ...envData,
      // 方法：env.get("VAR_NAME")
      get: (name: string): string | undefined => envData[name],
      // 方法：env.replace(text) - 替换 {{var}} 占位符
      replace: (text: string): string => {
        if (!text) return text;
        let result = text;
        for (const [name, value] of Object.entries(envData)) {
          const regex = new RegExp(`\\{\\{${name}\\}\\}`, 'g');
          result = result.replace(regex, value);
        }
        return result;
      },
      // 方法：env.all() - 获取所有环境变量
      all: (): Array<{ name: string; value: string }> => {
        return Object.entries(envData).map(([name, value]) => ({ name, value }));
      },
    };
    
    // 创建沙箱环境
    const sandbox = {
      payload: context.payload,
      topic: context.topic || "",
      env: envObject,
      console: {
        log: (...args: any[]) => console.log("[Script]", ...args),
        error: (...args: any[]) => console.error("[Script]", ...args),
        warn: (...args: any[]) => console.warn("[Script]", ...args),
      },
      JSON: JSON,
      parseInt: parseInt,
      parseFloat: parseFloat,
      String: String,
      Number: Number,
      Boolean: Boolean,
      Array: Array,
      Object: Object,
      Date: Date,
      Math: Math,
      encodeURIComponent: encodeURIComponent,
      decodeURIComponent: decodeURIComponent,
      atob: atob,
      btoa: btoa,
      // 加密工具
      crypto: {
        // 编码转换
        stringToBytes: CryptoUtils.stringToBytes.bind(CryptoUtils),
        bytesToString: CryptoUtils.bytesToString.bind(CryptoUtils),
        bytesToBase64: CryptoUtils.bytesToBase64.bind(CryptoUtils),
        base64ToBytes: CryptoUtils.base64ToBytes.bind(CryptoUtils),
        bytesToHex: CryptoUtils.bytesToHex.bind(CryptoUtils),
        hexToBytes: CryptoUtils.hexToBytes.bind(CryptoUtils),
        // 随机数
        randomBytes: CryptoUtils.randomBytes.bind(CryptoUtils),
        generateKey: CryptoUtils.generateKey.bind(CryptoUtils),
        generateIv: CryptoUtils.generateIv.bind(CryptoUtils),
        // 哈希
        sha256: CryptoUtils.sha256.bind(CryptoUtils),
        sha1: CryptoUtils.sha1.bind(CryptoUtils),
        md5: CryptoUtils.md5.bind(CryptoUtils),
        hmacSha256: CryptoUtils.hmacSha256.bind(CryptoUtils),
        // AES 加解密（Base64 格式）
        aesGcmEncrypt: CryptoUtils.aesGcmEncrypt.bind(CryptoUtils),
        aesGcmDecrypt: CryptoUtils.aesGcmDecrypt.bind(CryptoUtils),
        aesCbcEncrypt: CryptoUtils.aesCbcEncrypt.bind(CryptoUtils),
        aesCbcDecrypt: CryptoUtils.aesCbcDecrypt.bind(CryptoUtils),
        // AES 加解密（Hex 格式）
        aesGcmEncryptHex: CryptoUtils.aesGcmEncryptHex.bind(CryptoUtils),
        aesGcmDecryptHex: CryptoUtils.aesGcmDecryptHex.bind(CryptoUtils),
        aesCbcEncryptHex: CryptoUtils.aesCbcEncryptHex.bind(CryptoUtils),
        aesCbcDecryptHex: CryptoUtils.aesCbcDecryptHex.bind(CryptoUtils),
        // 其他
        xor: CryptoUtils.xor.bind(CryptoUtils),
        crc32: CryptoUtils.crc32.bind(CryptoUtils),
      },
    };

    // 包装代码，确保能够获取返回值
    // 如果代码定义了 process 函数，自动调用它
    const wrappedCode = `
      "use strict";
      ${code}
      // 如果定义了 process 函数，调用它并返回结果
      if (typeof process === 'function') {
        return await process(payload, topic);
      }
      return payload;
    `;

    // 使用 AsyncFunction 构造器支持 async/await
    // eslint-disable-next-line @typescript-eslint/no-implied-eval
    const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
    const fn = new AsyncFunction(
      ...Object.keys(sandbox),
      wrappedCode
    );

    // 执行异步函数并等待结果
    const result = await fn(...Object.values(sandbox));
    
    // 确保返回字符串
    if (result === undefined || result === null) {
      return context.payload;
    }
    
    if (typeof result === "string") {
      return result;
    }
    
    if (typeof result === "object") {
      return JSON.stringify(result);
    }
    
    return String(result);
  }

  /**
   * 验证脚本语法
   * @param code 脚本代码
   * @returns 错误信息或 null
   */
  static validateScript(code: string): string | null {
    try {
      new Function("payload", "topic", `"use strict"; ${code}`);
      return null;
    } catch (error: any) {
      return error.message || "脚本语法错误";
    }
  }
}
