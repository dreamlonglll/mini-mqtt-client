import { describe, it, expect } from "vitest";
import {
  bytesToBase64,
  base64ToBytes,
  bytesToHex,
  hexToBytes,
  isValidHex,
} from "@/utils/encoding";

describe("base64 / hex 严格编解码", () => {
  it("base64 往返得到原始字节", () => {
    const bytes = new Uint8Array([0, 1, 127, 128, 255, 65, 66]);
    expect(base64ToBytes(bytesToBase64(bytes))).toEqual(bytes);
  });

  it("大数组转 base64 不触发 RangeError", () => {
    const bytes = new Uint8Array(100 * 1024);
    for (let i = 0; i < bytes.length; i++) bytes[i] = i & 0xff;
    const base64 = bytesToBase64(bytes);
    expect(base64ToBytes(base64)).toEqual(bytes);
  });

  it("hex 往返得到原始字节", () => {
    const bytes = new Uint8Array([0x00, 0x0f, 0xa5, 0xff]);
    expect(bytesToHex(bytes)).toBe("000fa5ff");
    expect(hexToBytes("000fa5ff")).toEqual(bytes);
  });

  it("合法但含空白的 hex 正常解码", () => {
    expect(hexToBytes(" 0A 0b\n0C\t")).toEqual(new Uint8Array([0x0a, 0x0b, 0x0c]));
  });

  it("奇数长度的 hex 抛出中文错误", () => {
    expect(() => hexToBytes("0A0")).toThrowError(/偶数/);
  });

  it("含非法字符的 hex 抛出中文错误", () => {
    expect(() => hexToBytes("0AZZ")).toThrowError(/非法/);
  });

  it("空 hex 解码为空字节数组", () => {
    expect(hexToBytes("   ")).toEqual(new Uint8Array());
  });

  it("isValidHex 同时覆盖字符集与偶数长度", () => {
    expect(isValidHex("0a ff")).toBe(true);
    expect(isValidHex("0a f")).toBe(false);
    expect(isValidHex("zz")).toBe(false);
    expect(isValidHex("")).toBe(true);
  });
});
