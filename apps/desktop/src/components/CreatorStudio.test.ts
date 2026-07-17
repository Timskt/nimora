import { describe, expect, it } from "vitest";
import { assetDisplayName, formatBytes } from "./CreatorStudio";

const asset = {
  id: "character.example.mochi",
  assetType: "character" as const,
  version: "1.0.0",
  name: { en: "Mochi" },
  publisher: "publisher.example",
  license: "MIT",
  rendererBackend: "sprite-atlas" as const,
  fileCount: 3,
  totalBytes: 1_024,
};

describe("assetDisplayName", () => {
  it("prefers Chinese, then English, then package identity", () => {
    expect(assetDisplayName({ ...asset, name: { "zh-CN": "糯米", en: "Mochi" } })).toBe("糯米");
    expect(assetDisplayName(asset)).toBe("Mochi");
    expect(assetDisplayName({ ...asset, name: {} })).toBe(asset.id);
  });
});

describe("formatBytes", () => {
  it("formats package sizes without hiding precision", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1_536)).toBe("1.5 KiB");
    expect(formatBytes(1_572_864)).toBe("1.5 MiB");
  });
});
