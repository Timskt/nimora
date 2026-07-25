import { describe, expect, it } from "vitest";
import {
  describeEventBatch,
  describeSessionReceipt,
  describeSessionStatus,
  subscribingPrograms,
  summarizeEventExecution,
} from "./UserProgramEventSessionPanel";
import type {
  UserProgramCatalogEntry,
  UserProgramEventBatch,
  UserProgramEventExecutionReceipt,
  UserProgramEventSessionReceipt,
  UserProgramEventSessionStatus,
} from "../platform/desktop";

function entry(overrides: Partial<UserProgramCatalogEntry>): UserProgramCatalogEntry {
  return {
    programId: "studio.example.watcher",
    version: "1.0.0",
    capabilities: ["subscribe-events"],
    commands: [],
    subscriptions: ["pet.example.clicked"],
    timeoutMs: 1_000,
    memoryBytes: 4 * 1024 * 1024,
    permissionGranted: true,
    ...overrides,
  };
}

describe("subscribingPrograms", () => {
  it("keeps programs that declare subscriptions and the capability", () => {
    const eligible = subscribingPrograms([entry({})]);
    expect(eligible).toHaveLength(1);
  });

  it("drops programs without the subscribe-events capability", () => {
    const eligible = subscribingPrograms([entry({ capabilities: ["read-pet-state"] })]);
    expect(eligible).toHaveLength(0);
  });

  it("drops programs without any subscriptions", () => {
    const eligible = subscribingPrograms([entry({ subscriptions: [] })]);
    expect(eligible).toHaveLength(0);
  });
});

describe("describeSessionReceipt", () => {
  it("summarizes the opened session", () => {
    const receipt: UserProgramEventSessionReceipt = {
      subscriptionId: "018f0000-0000-7000-8000-0000000000ab",
      programId: "studio.example.watcher",
      version: "1.0.0",
      eventTypes: ["pet.a.one", "pet.a.two"],
      queueCapacity: 8,
    };
    const summary = describeSessionReceipt(receipt);
    expect(summary).toContain("studio.example.watcher");
    expect(summary).toContain("2 类事件");
    expect(summary).toContain("队列 8");
  });
});

describe("describeSessionStatus", () => {
  const base: UserProgramEventSessionStatus = {
    subscriptionId: "018f0000-0000-7000-8000-0000000000ac",
    programId: "studio.example.watcher",
    automatic: false,
    executed: 3,
    dropped: 1,
    lastError: null,
  };

  it("labels manual sessions with counters", () => {
    const summary = describeSessionStatus(base);
    expect(summary).toContain("手动");
    expect(summary).toContain("已执行 3");
    expect(summary).toContain("丢弃 1");
  });

  it("labels automatic sessions and surfaces the last error", () => {
    const summary = describeSessionStatus({ ...base, automatic: true, lastError: "boom" });
    expect(summary).toContain("自动循环中");
    expect(summary).toContain("最近错误：boom");
  });
});

describe("summarizeEventExecution", () => {
  it("reports an empty queue when nothing executed", () => {
    const receipt: UserProgramEventExecutionReceipt = { execution: null, dropped: 0 };
    expect(summarizeEventExecution(receipt)).toContain("暂无待处理事件");
  });

  it("reports dropped events when the queue was empty but overflowed", () => {
    const receipt: UserProgramEventExecutionReceipt = { execution: null, dropped: 2 };
    expect(summarizeEventExecution(receipt)).toContain("本轮丢弃 2");
  });

  it("counts responses and agent results on execution", () => {
    const receipt: UserProgramEventExecutionReceipt = {
      execution: {
        executionId: "018f0000-0000-7000-8000-0000000000ad",
        responses: [{} as never, {} as never],
        agentResults: [{} as never],
      },
      dropped: 0,
    };
    const summary = summarizeEventExecution(receipt);
    expect(summary).toContain("2 条能力响应");
    expect(summary).toContain("1 个 Agent 结果");
  });
});

describe("describeEventBatch", () => {
  it("labels an empty queue", () => {
    const batch: UserProgramEventBatch = { events: [], dropped: 0 };
    expect(describeEventBatch(batch)).toBe("队列为空");
  });

  it("counts drained events and drops", () => {
    const batch: UserProgramEventBatch = { events: [{} as never, {} as never], dropped: 1 };
    const summary = describeEventBatch(batch);
    expect(summary).toContain("排空 2 个事件");
    expect(summary).toContain("丢弃 1");
  });
});
