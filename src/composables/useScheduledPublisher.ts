import { computed, onScopeDispose, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePublishPipeline } from "@/composables/usePublishPipeline";
import type { CommandTemplate } from "@/stores/template";

/** 定时发布的调度参数 */
export interface ScheduledPublishConfig {
  /** 相邻两条命令之间的间隔（毫秒） */
  interval: number;
  /** 每轮之间的额外间隔（毫秒），0 表示不额外等待 */
  roundInterval: number;
  /** 命令顺序：按勾选顺序或按名称排序 */
  order: "selection" | "name";
  /** 循环方式：无限循环或固定轮数 */
  loopMode: "infinite" | "count";
  /** loopMode 为 count 时的轮数 */
  loopCount: number;
}

/** 一条发送日志 */
export interface ScheduledLogEntry {
  time: string;
  topic: string;
  payload: string;
  status: "success" | "error";
  message?: string;
}

/** 日志最多保留的条数，超出后丢弃最旧的一条 */
const MAX_LOGS = 100;

export interface ScheduledPublisherOptions {
  /** 目标 Server，调度时才读取 */
  serverId: () => number;
  /** 追加一条日志后触发（组件据此滚动日志到底部） */
  onLog?: () => void;
  /** 固定轮数模式跑完全部轮次时触发（组件据此提示用户） */
  onFinished?: () => void;
}

/**
 * 定时发布的调度状态机
 *
 * 负责"按什么节奏发、发到第几条第几轮、成败如何计数"，
 * 不关心命令怎么选出来（由调用方在 start 时传入）、界面怎么显示。
 * 每条命令都走统一发布管线，因此变量替换、脚本处理、写历史与手动发布完全一致。
 *
 * 命令列表在 start 时快照：运行期间外部增删模板不会让下标越界，
 * 也不会让本次运行的内容中途改变。
 */
export function useScheduledPublisher(options: ScheduledPublisherOptions) {
  const { locale } = useI18n();
  const { publish } = usePublishPipeline();

  const config = ref<ScheduledPublishConfig>({
    interval: 1000,
    roundInterval: 0,
    order: "selection",
    loopMode: "infinite",
    loopCount: 10,
  });

  /** 正在按节奏发送 */
  const isRunning = ref(false);
  /** 已结束但仍停留在运行视图（供用户查看结果） */
  const isCompleted = ref(false);
  const currentCommand = ref<CommandTemplate | null>(null);
  /** 本轮已发到第几条 */
  const currentIndex = ref(0);
  const currentRound = ref(1);
  const sentCount = ref(0);
  const successCount = ref(0);
  const failCount = ref(0);
  const logs = ref<ScheduledLogEntry[]>([]);
  /** 本次运行每轮的命令条数（start 时快照） */
  const totalPerRound = ref(0);

  /** 本次运行的命令快照 */
  let queue: CommandTemplate[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;

  const progressPercentage = computed(() => {
    if (totalPerRound.value === 0) return 0;
    return Math.round((currentIndex.value / totalPerRound.value) * 100);
  });

  const progressText = computed(() => `${currentIndex.value}/${totalPerRound.value}`);

  function appendLog(
    topic: string,
    payload: string,
    status: "success" | "error",
    message?: string
  ) {
    // 跟随当前界面语言格式化时间（在追加时读取，语言切换后立即生效）
    const time = new Date().toLocaleTimeString(locale.value, { hour12: false });
    logs.value.push({ time, topic, payload, status, message });
    if (logs.value.length > MAX_LOGS) {
      logs.value.shift();
    }
    options.onLog?.();
  }

  function clearTimer() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  }

  async function publishNext(): Promise<void> {
    if (!isRunning.value) return;
    if (queue.length === 0) {
      stop();
      return;
    }

    const command = queue[currentIndex.value];
    currentCommand.value = command;

    try {
      // 与手动发布共用同一条发布管线：变量替换 → 脚本 → 发布（HEX 由 Rust 解码）→ 写历史 → UI 入队
      const result = await publish({
        serverId: options.serverId(),
        topic: command.topic,
        payload: command.payload,
        qos: command.qos,
        retain: command.retain,
        format: command.payload_type as "json" | "hex" | "text",
      });

      if (result.success) {
        successCount.value++;
        appendLog(result.topic, result.payload, "success");
      } else {
        // 脚本失败或后端发布失败：本条计为失败，未处理的原文不会被发出
        failCount.value++;
        appendLog(command.topic, command.payload, "error", result.error);
      }
    } catch (error: any) {
      failCount.value++;
      appendLog(command.topic, command.payload, "error", error?.message);
    }

    sentCount.value++;
    currentIndex.value++;

    // 完成一轮
    if (currentIndex.value >= queue.length) {
      currentIndex.value = 0;

      // 固定轮数模式下达到轮数即结束
      if (config.value.loopMode === "count" && currentRound.value >= config.value.loopCount) {
        stop(true);
        options.onFinished?.();
        return;
      }

      currentRound.value++;

      // 轮间额外等待
      if (config.value.roundInterval > 0) {
        timer = setTimeout(() => void publishNext(), config.value.roundInterval);
        return;
      }
    }

    if (isRunning.value) {
      timer = setTimeout(() => void publishNext(), config.value.interval);
    }
  }

  /** 开始按当前配置发送给定的命令序列 */
  async function start(commands: CommandTemplate[]) {
    queue = [...commands];
    totalPerRound.value = queue.length;

    isRunning.value = true;
    isCompleted.value = false;
    currentIndex.value = 0;
    currentRound.value = 1;
    sentCount.value = 0;
    successCount.value = 0;
    failCount.value = 0;
    logs.value = [];

    await publishNext();
  }

  /**
   * 停止发送
   *
   * @param keepView 为 true 时置为"已完成"，界面停留在运行视图展示结果
   */
  function stop(keepView = false) {
    isRunning.value = false;
    if (keepView) {
      isCompleted.value = true;
    }
    clearTimer();
  }

  /** 清空本次运行的结果，回到可重新配置的状态 */
  function reset() {
    stop();
    isCompleted.value = false;
    currentCommand.value = null;
    currentIndex.value = 0;
    currentRound.value = 1;
    sentCount.value = 0;
    successCount.value = 0;
    failCount.value = 0;
    logs.value = [];
    totalPerRound.value = 0;
    queue = [];
  }

  function clearLogs() {
    logs.value = [];
  }

  // 组件卸载时确保定时器不残留
  onScopeDispose(() => clearTimer());

  return {
    config,
    isRunning,
    isCompleted,
    currentCommand,
    currentIndex,
    currentRound,
    sentCount,
    successCount,
    failCount,
    logs,
    progressPercentage,
    progressText,
    start,
    stop,
    reset,
    clearLogs,
  };
}
