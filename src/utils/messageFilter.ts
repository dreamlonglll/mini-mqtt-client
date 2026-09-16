import type { MqttMessage } from "@/types/mqtt";
import { getSearchText } from "./messageDerived";

export type DirectionFilter = "all" | "publish" | "receive";

/**
 * 创建一个带增量缓存的消息过滤器
 *
 * 消息数组只会"头插新消息 + 尾裁旧消息"，且消息对象不可变、id 单调递增。
 * 因此在过滤条件不变、数组实例不变的前提下，只需过滤新增的那几十条并拼到上次结果前面，
 * 再按存活的最旧 id 裁掉尾部，而不是每次 flush 都对全部消息重跑一遍字符串匹配。
 * 数组实例变化（切换 Server / 清空）或过滤条件变化时退回全量过滤。
 *
 * 无过滤条件时返回源数组的浅拷贝：源数组是就地更新的同一实例，
 * 虚拟滚动需要一个新的数组标识才能感知变化。
 */
export function createIncrementalMessageFilter() {
  let lastSource: MqttMessage[] | null = null;
  let lastKey = "";
  let lastResult: MqttMessage[] = [];
  let lastNewestId = -1;

  return function filterMessages(
    source: MqttMessage[],
    direction: DirectionFilter,
    rawKeyword: string
  ): MqttMessage[] {
    const keyword = rawKeyword.trim().toLowerCase();

    if (direction === "all" && !keyword) {
      lastSource = null;
      return source.slice();
    }

    const matches = (m: MqttMessage) =>
      (direction === "all" || m.direction === direction) &&
      (!keyword || getSearchText(m).includes(keyword));

    const key = `${direction} ${keyword}`;
    const newestId = source.length > 0 ? source[0].id! : -1;
    const oldestId = source.length > 0 ? source[source.length - 1].id! : -1;

    let result: MqttMessage[];
    if (lastSource === source && lastKey === key && newestId >= lastNewestId) {
      // 头部只过滤新增的消息
      const added: MqttMessage[] = [];
      for (const m of source) {
        if (m.id! <= lastNewestId) break;
        if (matches(m)) added.push(m);
      }
      // 尾部裁掉已经不在源数组里的旧消息
      let keep = lastResult.length;
      while (keep > 0 && lastResult[keep - 1].id! < oldestId) keep--;
      result = added.concat(keep === lastResult.length ? lastResult : lastResult.slice(0, keep));
    } else {
      result = source.filter(matches);
    }

    lastSource = source;
    lastKey = key;
    lastResult = result;
    lastNewestId = newestId;
    return result;
  };
}
