import { describe, expect, it } from "vitest";
import { providerDraftIsValid, secretReferenceForProvider, validateProviderDraft } from "./ProviderSettings";
import type { UpsertOpenAiProviderRequest } from "../platform/desktop";

describe("ProviderSettings", () => {
  it("derives a provider-specific system secret reference", () => {
    expect(secretReferenceForProvider("provider:openai-compatible:team")).toBe(
      "secret:provider-openai-compatible-team",
    );
    expect(secretReferenceForProvider("provider:openai-compatible:private")).not.toBe(
      secretReferenceForProvider("provider:openai-compatible:team"),
    );
  });
});

function draftFixture(overrides: Partial<UpsertOpenAiProviderRequest> = {}): UpsertOpenAiProviderRequest {
  return {
    id: "provider:openai-compatible:team",
    displayName: "团队网关",
    baseUrl: "https://api.example.com",
    credentialReference: "secret:provider-openai-compatible-team",
    defaultModel: "gpt-4.1-mini",
    contextWindowTokens: 128_000,
    maxOutputTokens: 8_192,
    reasoning: null,
    enabled: true,
    revision: 0,
    ...overrides,
  };
}

describe("validateProviderDraft", () => {
  it("accepts a well-formed HTTPS provider draft", () => {
    const errors = validateProviderDraft(draftFixture());
    expect(errors).toEqual({});
    expect(providerDraftIsValid(errors)).toBe(true);
  });

  it("requires a display name and a namespaced provider id", () => {
    expect(validateProviderDraft(draftFixture({ displayName: "   " })).displayName).toBe("请填写显示名称");
    expect(validateProviderDraft(draftFixture({ id: "openai" })).id).toContain("provider:");
    expect(validateProviderDraft(draftFixture({ id: "provider:好" })).id).toContain("provider:");
  });

  it("blocks public plaintext endpoints while allowing loopback http", () => {
    expect(validateProviderDraft(draftFixture({ baseUrl: "http://api.example.com" })).baseUrl).toBe("公网地址必须使用 HTTPS");
    expect(validateProviderDraft(draftFixture({ baseUrl: "http://localhost:11434" })).baseUrl).toBeUndefined();
    expect(validateProviderDraft(draftFixture({ baseUrl: "not a url" })).baseUrl).toBe("仅支持 http(s) 地址");
  });

  it("enforces sane token floors", () => {
    expect(validateProviderDraft(draftFixture({ contextWindowTokens: 0 })).contextWindowTokens).toBe("至少 1024");
    expect(validateProviderDraft(draftFixture({ maxOutputTokens: 0 })).maxOutputTokens).toBe("至少 128");
    expect(providerDraftIsValid(validateProviderDraft(draftFixture({ maxOutputTokens: 0 })))).toBe(false);
  });
});
