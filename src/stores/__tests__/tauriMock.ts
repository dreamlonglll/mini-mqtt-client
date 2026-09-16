import { vi } from "vitest";
import { encodeMessageFrames, type ReceivedMessage } from "@/utils/messageFrame";

type EventHandler = (event: { payload: any }) => void;

/** 被 mock 的 invoke，测试内用 mockImplementation 指定各命令的返回值 */
export const invokeMock = vi.fn();

const eventHandlers = new Map<string, EventHandler[]>();

/** 被 mock 的 listen，记录事件处理器供测试触发 */
export const listenMock = vi.fn(async (name: string, handler: EventHandler) => {
  const handlers = eventHandlers.get(name) ?? [];
  handlers.push(handler);
  eventHandlers.set(name, handlers);
  return () => {
    const list = eventHandlers.get(name);
    if (list) {
      eventHandlers.set(
        name,
        list.filter((h) => h !== handler)
      );
    }
  };
});

/**
 * 替代 `@tauri-apps/api/core` 的 Channel：只保留 onmessage 与 IPC 序列化形态，
 * 测试通过 `register_message_channel` 的调用参数拿到实例后直接调用 onmessage
 */
export class ChannelMock<T = unknown> {
  onmessage: (data: T) => void;

  constructor(onmessage?: (data: T) => void) {
    this.onmessage = onmessage ?? (() => {});
  }

  toJSON() {
    return "__CHANNEL__:mock";
  }
}

/** 模拟后端 emit 一次事件 */
export function emitTauriEvent(name: string, payload: any) {
  for (const handler of [...(eventHandlers.get(name) ?? [])]) {
    handler({ payload });
  }
}

/** 最近一次注册到后端的消息 Channel */
export function latestMessageChannel(): ChannelMock<ArrayBuffer> | undefined {
  const registrations = invokeArgsOf("register_message_channel");
  return registrations.length > 0 ? registrations[registrations.length - 1].channel : undefined;
}

/** 模拟后端经 Channel 推送一帧原始字节 */
export function emitRawMessageFrame(frame: ArrayBuffer) {
  const channel = latestMessageChannel();
  if (!channel) {
    throw new Error("消息 Channel 尚未注册，请先调用 initListeners");
  }
  channel.onmessage(frame);
}

/** 模拟后端推送一批消息（按帧格式编码后经 Channel 送达） */
export function emitMessageBatch(messages: ReceivedMessage[]) {
  emitRawMessageFrame(encodeMessageFrames(messages));
}

/** 统计某个命令被 invoke 的次数 */
export function invokeCountOf(command: string): number {
  return invokeMock.mock.calls.filter(([cmd]) => cmd === command).length;
}

/** 取某个命令的全部调用参数 */
export function invokeArgsOf(command: string): any[] {
  return invokeMock.mock.calls.filter(([cmd]) => cmd === command).map(([, args]) => args);
}

/** 重置 invoke / listen 的记录 */
export function resetTauriMock() {
  invokeMock.mockReset();
  listenMock.mockClear();
  eventHandlers.clear();
}
