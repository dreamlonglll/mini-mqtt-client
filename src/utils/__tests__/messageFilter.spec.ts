import { describe, it, expect } from "vitest";
import { createIncrementalMessageFilter } from "@/utils/messageFilter";
import type { MqttMessage } from "@/types/mqtt";

let nextId = 1;

function message(topic: string, text: string, direction: "publish" | "receive" = "receive"): MqttMessage {
  return {
    id: nextId++,
    server_id: 1,
    direction,
    topic,
    payload: new TextEncoder().encode(text),
    qos: 0,
    retain: false,
    timestamp: 0,
  };
}

/** 模拟 store 的 flush：头插新消息并按上限裁尾，保持同一数组实例 */
function flush(list: MqttMessage[], incoming: MqttMessage[], limit = Infinity) {
  list.unshift(...incoming.reverse());
  if (list.length > limit) list.length = limit;
}

describe("增量消息过滤", () => {
  it("无过滤条件时返回源数组的浅拷贝", () => {
    const filter = createIncrementalMessageFilter();
    const list = [message("a", "1"), message("b", "2")];
    const result = filter(list, "all", "  ");
    expect(result).not.toBe(list);
    expect(result).toEqual(list);
  });

  it("按方向与关键词过滤，关键词不区分大小写并匹配 topic 与内容", () => {
    const filter = createIncrementalMessageFilter();
    const list = [
      message("Dev/Temp", "hello", "receive"),
      message("dev/hum", "WORLD", "publish"),
      message("other", "temp inside", "receive"),
    ];
    expect(filter(list, "all", "TEMP").map((m) => m.topic)).toEqual(["Dev/Temp", "other"]);
    expect(filter(list, "publish", "").map((m) => m.topic)).toEqual(["dev/hum"]);
    expect(filter(list, "receive", "world")).toEqual([]);
  });

  it("头插新消息后只过滤新增部分，旧结果原样复用", () => {
    const filter = createIncrementalMessageFilter();
    const list: MqttMessage[] = [];
    flush(list, [message("x/1", "match"), message("x/2", "skip")]);
    const first = filter(list, "all", "match");
    expect(first.map((m) => m.topic)).toEqual(["x/1"]);

    // 篡改旧消息的搜索缓存：若增量路径重新匹配旧消息，它就会被剔除
    first[0].searchText = "nothing here";

    flush(list, [message("x/3", "MATCH"), message("x/4", "skip")]);
    const second = filter(list, "all", "match");
    expect(second.map((m) => m.topic)).toEqual(["x/3", "x/1"]);
    expect(second).not.toBe(first);
  });

  it("尾部被裁掉的消息从结果里移除", () => {
    const filter = createIncrementalMessageFilter();
    const list: MqttMessage[] = [];
    flush(list, [message("a", "k"), message("b", "k"), message("c", "k")]);
    expect(filter(list, "all", "k").map((m) => m.topic)).toEqual(["c", "b", "a"]);

    // 上限 3：新来 2 条后最旧的 a、b 被裁掉
    flush(list, [message("d", "k"), message("e", "x")], 3);
    expect(list.map((m) => m.topic)).toEqual(["e", "d", "c"]);
    expect(filter(list, "all", "k").map((m) => m.topic)).toEqual(["d", "c"]);
  });

  it("数组实例或过滤条件变化时退回全量过滤", () => {
    const filter = createIncrementalMessageFilter();
    const list = [message("a", "k")];
    expect(filter(list, "all", "k")).toHaveLength(1);

    // 条件变化
    expect(filter(list, "publish", "k")).toHaveLength(0);
    expect(filter(list, "all", "k")).toHaveLength(1);

    // 清空后换了实例
    const other = [message("z", "k"), message("y", "k")];
    expect(filter(other, "all", "k").map((m) => m.topic)).toEqual(["z", "y"]);
  });

  it("增量结果与全量过滤结果一致", () => {
    const incremental = createIncrementalMessageFilter();
    const full = createIncrementalMessageFilter();
    const list: MqttMessage[] = [];
    for (let round = 0; round < 20; round++) {
      const incoming = Array.from({ length: 7 }, (_, i) =>
        message(`t/${round}/${i}`, i % 3 === 0 ? "needle" : "hay", i % 2 ? "publish" : "receive")
      );
      flush(list, incoming, 50);
      const a = incremental(list, "receive", "needle");
      const b = full(list.slice(), "receive", "needle");
      expect(a.map((m) => m.id)).toEqual(b.map((m) => m.id));
    }
  });
});
