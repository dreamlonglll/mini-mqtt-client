import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { effectScope, type EffectScope } from "vue";

// 状态机只用 locale 做时间格式化，测试里给一个固定语言即可
vi.mock("vue-i18n", () => ({
  useI18n: () => ({ locale: { value: "zh-CN" } }),
}));

const publishMock = vi.fn();
vi.mock("@/composables/usePublishPipeline", () => ({
  usePublishPipeline: () => ({ publish: publishMock }),
}));

import {
  useScheduledPublisher,
  type ScheduledPublisherOptions,
} from "@/composables/useScheduledPublisher";

const SERVER_ID = 1;
const INTERVAL = 1000;

const scopes: EffectScope[] = [];

/** 在独立的 effect scope 里创建状态机（onScopeDispose 需要宿主 scope） */
function createPublisher(options: Partial<ScheduledPublisherOptions> = {}) {
  const scope = effectScope();
  scopes.push(scope);
  const publisher = scope.run(() =>
    useScheduledPublisher({ serverId: () => SERVER_ID, ...options })
  )!;
  publisher.config.value.interval = INTERVAL;
  return publisher;
}

function command(id: number, name: string) {
  return {
    id,
    server_id: SERVER_ID,
    name,
    topic: `dev/${name}`,
    payload: `payload-${id}`,
    payload_type: "text",
    qos: 0,
    retain: false,
    use_count: 0,
  } as any;
}

/** 发出去的 topic 序列，用来断言顺序 */
function publishedTopics(): string[] {
  return publishMock.mock.calls.map((call) => call[0].topic);
}

describe("定时发布调度状态机", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    publishMock.mockReset();
    publishMock.mockImplementation(async (req: any) => ({
      success: true,
      topic: req.topic,
      payload: req.payload,
    }));
  });

  afterEach(() => {
    scopes.splice(0).forEach((scope) => scope.stop());
    vi.useRealTimers();
  });

  it("按配置的间隔逐条发送，顺序与传入一致", async () => {
    const publisher = createPublisher();

    await publisher.start([command(1, "a"), command(2, "b")]);
    // 首条立即发出，后续条目等间隔
    expect(publishedTopics()).toEqual(["dev/a"]);

    await vi.advanceTimersByTimeAsync(INTERVAL - 1);
    expect(publishedTopics()).toEqual(["dev/a"]);

    await vi.advanceTimersByTimeAsync(1);
    expect(publishedTopics()).toEqual(["dev/a", "dev/b"]);
    expect(publisher.sentCount.value).toBe(2);
    expect(publisher.successCount.value).toBe(2);
    expect(publisher.isRunning.value).toBe(true);
  });

  it("固定轮数模式跑满轮数后停止，并停留在结果视图", async () => {
    const onFinished = vi.fn();
    const publisher = createPublisher({ onFinished });
    publisher.config.value.loopMode = "count";
    publisher.config.value.loopCount = 2;

    await publisher.start([command(1, "a"), command(2, "b")]);
    // 2 条命令 × 2 轮 = 4 次发送
    await vi.advanceTimersByTimeAsync(INTERVAL * 3);

    expect(publishedTopics()).toEqual(["dev/a", "dev/b", "dev/a", "dev/b"]);
    expect(publisher.isRunning.value).toBe(false);
    expect(publisher.isCompleted.value).toBe(true);
    expect(onFinished).toHaveBeenCalledTimes(1);

    // 停止后不应再有新的发送被调度
    await vi.advanceTimersByTimeAsync(INTERVAL * 5);
    expect(publishMock).toHaveBeenCalledTimes(4);
  });

  it("轮间隔按 roundInterval 等待，不使用条目间隔", async () => {
    const publisher = createPublisher();
    publisher.config.value.roundInterval = 5000;

    await publisher.start([command(1, "a")]);
    // 单条命令发完即为一轮，轮次已推进
    expect(publishedTopics()).toEqual(["dev/a"]);
    expect(publisher.currentRound.value).toBe(2);

    // 此时应等 roundInterval 而不是条目间隔
    await vi.advanceTimersByTimeAsync(INTERVAL);
    expect(publishMock).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(5000 - INTERVAL);
    expect(publishMock).toHaveBeenCalledTimes(2);
  });

  it("单条发布失败计为失败并继续下一条", async () => {
    publishMock.mockReset();
    publishMock
      .mockResolvedValueOnce({
        success: false,
        topic: "dev/a",
        payload: "payload-1",
        error: "脚本执行失败",
      })
      .mockImplementation(async (req: any) => ({
        success: true,
        topic: req.topic,
        payload: req.payload,
      }));

    const publisher = createPublisher();
    await publisher.start([command(1, "a"), command(2, "b")]);
    await vi.advanceTimersByTimeAsync(INTERVAL);

    expect(publisher.failCount.value).toBe(1);
    expect(publisher.successCount.value).toBe(1);
    expect(publisher.sentCount.value).toBe(2);
    expect(publisher.logs.value.map((log) => log.status)).toEqual(["error", "success"]);
    expect(publisher.logs.value[0].message).toBe("脚本执行失败");
  });

  it("发布抛异常时计为失败且调度链不中断", async () => {
    publishMock.mockReset();
    publishMock
      .mockRejectedValueOnce(new Error("boom"))
      .mockImplementation(async (req: any) => ({
        success: true,
        topic: req.topic,
        payload: req.payload,
      }));

    const publisher = createPublisher();
    await publisher.start([command(1, "a"), command(2, "b")]);
    await vi.advanceTimersByTimeAsync(INTERVAL);

    expect(publisher.failCount.value).toBe(1);
    expect(publisher.logs.value[0].message).toBe("boom");
    // 一条异常不能让整条调度链静默死亡
    expect(publishMock).toHaveBeenCalledTimes(2);
    expect(publisher.isRunning.value).toBe(true);
  });

  it("命令序列在 start 时快照，运行中外部改动不影响本次运行", async () => {
    const commands = [command(1, "a"), command(2, "b")];
    const publisher = createPublisher();

    await publisher.start(commands);
    // 模拟运行期间模板被外部删除
    commands.length = 0;

    await vi.advanceTimersByTimeAsync(INTERVAL);
    expect(publishedTopics()).toEqual(["dev/a", "dev/b"]);
    expect(publisher.isRunning.value).toBe(true);
  });

  it("stop 后不再调度新的发送", async () => {
    const publisher = createPublisher();
    await publisher.start([command(1, "a"), command(2, "b")]);

    publisher.stop();
    await vi.advanceTimersByTimeAsync(INTERVAL * 5);

    expect(publishMock).toHaveBeenCalledTimes(1);
    expect(publisher.isRunning.value).toBe(false);
    // 未要求保留视图时不置为已完成
    expect(publisher.isCompleted.value).toBe(false);
  });

  it("reset 清空计数与日志并退出结果视图", async () => {
    const publisher = createPublisher();
    await publisher.start([command(1, "a")]);
    publisher.stop(true);
    expect(publisher.isCompleted.value).toBe(true);

    publisher.reset();

    expect(publisher.isCompleted.value).toBe(false);
    expect(publisher.sentCount.value).toBe(0);
    expect(publisher.successCount.value).toBe(0);
    expect(publisher.failCount.value).toBe(0);
    expect(publisher.currentRound.value).toBe(1);
    expect(publisher.logs.value).toEqual([]);
    expect(publisher.progressPercentage.value).toBe(0);
  });

  it("scope 销毁时挂起的定时器被清理", async () => {
    const scope = effectScope();
    const publisher = scope.run(() =>
      useScheduledPublisher({ serverId: () => SERVER_ID })
    )!;
    publisher.config.value.interval = INTERVAL;

    await publisher.start([command(1, "a"), command(2, "b")]);
    scope.stop();

    await vi.advanceTimersByTimeAsync(INTERVAL * 5);
    expect(publishMock).toHaveBeenCalledTimes(1);
  });
});
