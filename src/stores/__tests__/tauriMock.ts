import { vi } from "vitest";

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

/** 模拟后端 emit 一次事件 */
export function emitTauriEvent(name: string, payload: any) {
  for (const handler of [...(eventHandlers.get(name) ?? [])]) {
    handler({ payload });
  }
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
