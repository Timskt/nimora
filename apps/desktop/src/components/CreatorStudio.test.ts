import { describe, expect, it } from "vitest";
import { assetDisplayName } from "./CreatorStudio";

const asset = {
  id: "character.example.mochi",
  assetType: "character" as const,
  version: "1.0.0",
  name: { en: "Mochi" },
  rendererBackend: "sprite-atlas" as const,
};

describe("assetDisplayName", () => {
  it("prefers Chinese, then English, then package identity", () => {
    expect(assetDisplayName({ ...asset, name: { "zh-CN": "糯米", en: "Mochi" } })).toBe("糯米");
    expect(assetDisplayName(asset)).toBe("Mochi");
    expect(assetDisplayName({ ...asset, name: {} })).toBe(asset.id);
  });
});
