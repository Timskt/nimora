import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CapabilityGapPreview, ThemeDraftPreview } from "./AiCreatorWorkspace";
import { CapabilityProposalGovernance } from "./CapabilityProposalGovernance";
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
        availableSemanticInputs: ["perception.gesture-request"],
        requiredSemanticOutputs: ["perception.gesture-event"],
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
      semanticCompositionPlan: {
        spec: "nimora.capability-semantic-plan/1",
        graphDigest: "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        capabilityPath: [], availableOutputs: [], missingOutputs: ["perception.gesture-event"],
        totalCostUnits: 0, fullyResolved: false, expandedStates: 1,
      },
      usage: { inputTokens: 12, outputTokens: 18, costMicrounits: 0 },
      finishReason: "stop",
    };

    const markup = renderToStaticMarkup(<CapabilityGapPreview disabled={false} gap={result.capabilityGap!} onSave={() => undefined} onSubmitProposal={() => undefined} result={result} saveNotice={null} />);
    expect(markup).toContain("CAPABILITY GAP · NON-EXECUTABLE");
    expect(markup).toContain("perception.camera.observe");
    expect(markup).toContain("需要平台提案");
    expect(markup).toContain("宿主双重核验");
    expect(markup).toContain("perception.gesture-event");
    expect(markup).toContain("不证明自然语言映射绝对完整");
    expect(markup).toContain("提交平台能力提案");
    expect(markup).toContain("不会创建 Handler");
    expect(markup).not.toContain("批准此权限与行为审查");
    expect(markup).not.toContain("原子安装");
    expect(markup).toContain("保存缺口报告");
    expect(markup).not.toContain("保存到 Workspace");
  });
});

describe("ThemeDraftPreview", () => {
  it("renders validated tokens inside an inert local preview", () => {
    const markup = renderToStaticMarkup(<ThemeDraftPreview metadata={{
      id: "theme.local.aurora",
      version: "1.0.0",
      name: { "zh-CN": "极光夜航" },
      publisher: "publisher.local.user",
      license: "LicenseRef-Proprietary",
      theme: {
        spec: "nimora.theme/1",
        mode: "dark",
        colors: {
          surface: "#171922", surfaceElevated: "#222532", text: "#f5f6fb",
          textMuted: "#b4b8c8", accent: "#9f91ff", accentSoft: "#312d50",
          border: "#3d4152", success: "#87c98a", danger: "#ee8c85",
        },
        cornerStyle: "rounded",
        motion: "reduced",
      },
    }} />);

    expect(markup).toContain("极光夜航");
    expect(markup).toContain("dark · reduced motion");
    expect(markup).toContain("--preview-surface:#171922");
    expect(markup).toContain("--preview-accent:#9f91ff");
    expect(markup).toContain("安装不会自动改变当前主题");
    expect(markup).not.toContain("<script");
  });
});

describe("CapabilityProposalGovernance", () => {
  it("renders the inert review boundary before a workspace is selected", () => {
    const markup = renderToStaticMarkup(<CapabilityProposalGovernance disabled={false} />);

    expect(markup).toContain("平台能力提案治理");
    expect(markup).toContain("打开提案 Workspace");
    expect(markup).toContain("不会创建 Handler");
    expect(markup).toContain("不代表能力已实现");
    expect(markup).not.toContain("维护者裁决理由");
  });
});
