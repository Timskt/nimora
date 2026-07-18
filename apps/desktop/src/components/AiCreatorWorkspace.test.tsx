import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CapabilityGapPreview } from "./AiCreatorWorkspace";
import type { CreatorDraftResult } from "../platform/desktop";

describe("CapabilityGapPreview", () => {
  it("renders an inert gap without draft approval or installation actions", () => {
    const result: CreatorDraftResult = {
      spec: "nimora.desktop-creator-draft/1",
      outcome: "capability-gap",
      task: { id: "018f0000-0000-7000-8000-000000000031", status: "completed", providerId: "provider:test" },
      draft: null,
      capabilityGap: {
        spec: "nimora.capability-gap/1",
        title: "缺少摄像头观察能力",
        summary: "当前 Registry 无法表达该目标。",
        requestedOutcome: "识别获准手势后播放动作。",
        missingCapabilities: [{
          capability: "perception.camera.observe",
          reason: "尚无持续同意约束的摄像头观察能力。",
          requiredOperations: ["生成不保留原始帧的手势事件。"],
        }],
        closestAlternatives: [{ kind: "automation", title: "手动触发动作", tradeoff: "需要用户主动操作。" }],
        platformProposalRequired: true,
      },
      catalogDigest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      compositionGraphDigest: "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
      compositionPlan: {
        spec: "nimora.capability-composition-plan/1",
        catalogDigest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        requestedCapabilities: ["perception.camera.observe"],
        resolvedCapabilities: [],
        missingCapabilities: ["perception.camera.observe"],
        fullyResolved: false,
      },
      usage: { inputTokens: 12, outputTokens: 18, costMicrounits: 0 },
      finishReason: "stop",
    };

    const markup = renderToStaticMarkup(<CapabilityGapPreview disabled={false} gap={result.capabilityGap!} onSave={() => undefined} result={result} saveNotice={null} />);
    expect(markup).toContain("CAPABILITY GAP · NON-EXECUTABLE");
    expect(markup).toContain("perception.camera.observe");
    expect(markup).toContain("需要平台提案");
    expect(markup).toContain("宿主目录已核验");
    expect(markup).toContain("语义组合图");
    expect(markup).toContain("未证明自然语言目标不存在其他组合路径");
    expect(markup).not.toContain("批准此权限与行为审查");
    expect(markup).not.toContain("原子安装");
    expect(markup).toContain("保存缺口报告");
    expect(markup).not.toContain("保存到 Workspace");
  });
});
