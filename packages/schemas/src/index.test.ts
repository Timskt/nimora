import { describe, expect, it } from "vitest";
import {
  assetPackageSchema,
  assetManifestSchema,
  eventSchema,
  petSchema,
  pointerButtonSchema,
  profileSnapshotSchema,
  safetySnapshotSchema,
} from "./index";

const validAssetManifest = {
  spec: "nimora.asset/1",
  id: "character.example.mochi",
  type: "character",
  version: "1.0.0",
  name: { "zh-CN": "糯米", en: "Mochi" },
  publisher: "publisher.example",
  license: "LicenseRef-Commercial",
  engines: { nimora: ">=0.1.0 <1.0.0" },
  render: {
    backend: "sprite-atlas",
    canvas: { width: 512, height: 512 },
    anchor: { x: 0.5, y: 1 },
    defaultScale: 0.5,
    pixelArt: false,
  },
  entrypoints: { animationGraph: "animations/graph.json" },
  capabilities: ["pet.walk", "pet.drag"],
  fallbacks: { "pet.happy": "pet.idle" },
  locales: ["zh-CN", "en"],
  integrity: { algorithm: "sha256", files: "integrity.json" },
};

describe("eventSchema", () => {
  it("accepts the versioned event wire format", () => {
    const result = eventSchema.safeParse({
      spec: "nimora.event/1",
      id: "019bf2c6-4d40-7000-8000-000000000001",
      eventType: "pet.state.changed",
      source: "core",
      timestamp: "2026-07-17T01:00:00Z",
      traceId: "019bf2c6-4d40-7000-8000-000000000002",
      data: { state: "idle" },
    });
    expect(result.success).toBe(true);
  });

  it("rejects unqualified event types", () => {
    const result = eventSchema.safeParse({
      spec: "nimora.event/1",
      id: "019bf2c6-4d40-7000-8000-000000000001",
      eventType: "clicked",
      source: "core",
      timestamp: "2026-07-17T01:00:00Z",
      traceId: "019bf2c6-4d40-7000-8000-000000000002",
      data: null,
    });
    expect(result.success).toBe(false);
  });
});

describe("petSchema", () => {
  it("rejects vitals outside the domain range", () => {
    const result = petSchema.safeParse({
      id: "019bf2c6-4d40-7000-8000-000000000001",
      name: "Aster",
      state: "idle",
      emotion: "happy",
      position: { x: 0, y: 0 },
      energy: 101,
      mood: 70,
      affinity: 0,
    });
    expect(result.success).toBe(false);
  });
});

describe("profileSnapshotSchema", () => {
  it("accepts a versioned active profile collection", () => {
    const id = "019bf2c6-4d40-7000-8000-000000000010";
    expect(profileSnapshotSchema.safeParse({
      schemaVersion: 1,
      activeProfileId: id,
      profiles: [{
        id,
        name: "Focus",
        policy: {
          mode: "focus",
          alwaysOnTop: true,
          clickThrough: false,
          soundEnabled: true,
          proactiveFrequency: 25,
        },
      }],
    }).success).toBe(true);
  });

  it("rejects policy values outside the domain range", () => {
    const id = "019bf2c6-4d40-7000-8000-000000000010";
    expect(profileSnapshotSchema.safeParse({
      schemaVersion: 1,
      activeProfileId: id,
      profiles: [{
        id,
        name: "Focus",
        policy: {
          mode: "work",
          alwaysOnTop: null,
          clickThrough: null,
          soundEnabled: null,
          proactiveFrequency: 101,
        },
      }],
    }).success).toBe(false);
  });
});

describe("safetySnapshotSchema", () => {
  it("keeps runtime mode and reason consistent", () => {
    expect(safetySnapshotSchema.safeParse({ mode: "safe", reason: "manual" }).success).toBe(true);
    expect(safetySnapshotSchema.safeParse({ mode: "normal", reason: null }).success).toBe(true);
    expect(safetySnapshotSchema.safeParse({ mode: "safe", reason: null }).success).toBe(false);
    expect(safetySnapshotSchema.safeParse({ mode: "normal", reason: "crash_loop" }).success).toBe(false);
  });
});

describe("pointerButtonSchema", () => {
  it("accepts portable pointer buttons only", () => {
    expect(pointerButtonSchema.safeParse("left").success).toBe(true);
    expect(pointerButtonSchema.safeParse("touch").success).toBe(false);
  });
});

describe("assetManifestSchema", () => {
  it("accepts a versioned character manifest", () => {
    expect(assetManifestSchema.safeParse(validAssetManifest).success).toBe(true);
  });

  it("rejects package escape paths and unsupported renderers", () => {
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      render: { ...validAssetManifest.render, backend: "obj" },
      entrypoints: { animationGraph: "../outside.json" },
    }).success).toBe(false);
  });
});

describe("assetPackageSchema", () => {
  it("requires a complete, deduplicated integrity inventory", () => {
    const result = assetPackageSchema.safeParse({
      manifest: validAssetManifest,
      files: [
        { path: "manifest.json", sha256: "a".repeat(64), bytes: 10, mediaType: "application/json" },
        { path: "preview/poster.webp", sha256: "b".repeat(64), bytes: 20, mediaType: "image/webp" },
      ],
      dependencies: [],
      totalBytes: 30,
    });
    expect(result.success).toBe(true);
  });

  it("rejects duplicate files and inconsistent totals", () => {
    const result = assetPackageSchema.safeParse({
      manifest: validAssetManifest,
      files: [
        { path: "manifest.json", sha256: "a".repeat(64), bytes: 10, mediaType: "application/json" },
        { path: "manifest.json", sha256: "b".repeat(64), bytes: 20, mediaType: "application/json" },
      ],
      totalBytes: 10,
    });
    expect(result.success).toBe(false);
  });
});
