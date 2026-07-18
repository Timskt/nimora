import { describe, expect, it } from "vitest";
import { secretReferenceForProvider } from "./ProviderSettings";

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
