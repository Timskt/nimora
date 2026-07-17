import { describe, expect, it } from "vitest";
import { createImportPlan } from "./index";

const manifest = {
  spec: "nimora.asset/1" as const,
  id: "character.example.mochi",
  type: "character" as const,
  version: "1.0.0",
  name: { en: "Mochi" },
  publisher: "publisher.example",
  license: "MIT",
  engines: { nimora: ">=0.1.0" },
  capabilities: [],
  fallbacks: {},
  locales: ["en"],
  integrity: { algorithm: "sha256" as const, files: "integrity.json" },
};

const entry = {
  path: "manifest.json",
  sha256: "a".repeat(64),
  bytes: 12,
  mediaType: "application/json",
};

describe("createImportPlan", () => {
  it("creates a side-effect-free plan from an isolated inventory", () => {
    const result = createImportPlan(manifest, [entry]);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.plan.package.totalBytes).toBe(12);
      expect(result.plan.files[0]?.path).toBe("manifest.json");
    }
  });

  it("rejects traversal entries before installation", () => {
    const result = createImportPlan(manifest, [{ ...entry, path: "../escape.json" }]);
    expect(result).toMatchObject({ ok: false, error: { code: "invalid_inventory" } });
  });
});
