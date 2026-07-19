import { describe, expect, it } from "vitest";
import {
  assetPackageSchema,
  assetManifestSchema,
  eventSchema,
  modelAnimationMapSchema,
  petSchema,
  petRelationshipSnapshotSchema,
  pointerButtonSchema,
  profileSnapshotSchema,
  safetySnapshotSchema,
  spriteClipsSchema,
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
  entrypoints: {
    animationGraph: "animations/graph.json",
    clips: "animations/clips.json",
    previewPoster: "preview/poster.webp",
  },
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

describe("petRelationshipSnapshotSchema", () => {
  const relationship = {
    bondPoints: 84,
    affinity: 84,
    level: 2,
    levelProgress: 34,
    pointsPerLevel: 50,
    stage: "familiar",
    nextStage: "trusted",
    nextStageAt: 100,
  };

  it("accepts an authoritative relationship projection", () => {
    expect(petRelationshipSnapshotSchema.safeParse(relationship).success).toBe(true);
  });

  it("rejects unknown stages and unsafe counters", () => {
    expect(petRelationshipSnapshotSchema.safeParse({ ...relationship, stage: "best_friend" }).success).toBe(false);
    expect(petRelationshipSnapshotSchema.safeParse({ ...relationship, bondPoints: Number.MAX_SAFE_INTEGER + 1 }).success).toBe(false);
  });
});

describe("modelAnimationMapSchema", () => {
  it("accepts named loop and one-shot model animations", () => {
    expect(modelAnimationMapSchema.safeParse({
      spec: "nimora.animation-map/1",
      clips: {
        "pet.idle": { animation: "Idle", looped: true },
        "pet.click": { animation: "Wave", looped: false },
      },
    }).success).toBe(true);
  });

  it("rejects missing idle and invalid action identifiers", () => {
    expect(modelAnimationMapSchema.safeParse({
      spec: "nimora.animation-map/1",
      clips: { click: { animation: "Wave", looped: false } },
    }).success).toBe(false);
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

  it("defaults legacy care needs to a healthy low-pressure baseline", () => {
    const result = petSchema.parse({
      id: "019bf2c6-4d40-7000-8000-000000000001",
      name: "Aster",
      state: "idle",
      emotion: "happy",
      position: { x: 0, y: 0 },
      energy: 80,
      mood: 70,
      affinity: 0,
    });
    expect(result.satiety).toBe(100);
    expect(result.cleanliness).toBe(100);
    expect(result.homePosition).toBeUndefined();
    expect(result.inventory).toEqual([
      { itemId: "berry_bite", quantity: 3 },
      { itemId: "star_ball", quantity: 3 },
      { itemId: "bubble_soap", quantity: 3 },
    ]);
  });

  it("accepts a finite optional home anchor", () => {
    const pet = {
      id: "019bf2c6-4d40-7000-8000-000000000001",
      name: "Aster",
      state: "idle",
      emotion: "happy",
      position: { x: 0, y: 0 },
      homePosition: { x: 120, y: 80 },
      energy: 80,
      mood: 70,
      affinity: 0,
    };
    expect(petSchema.safeParse(pet).success).toBe(true);
    expect(petSchema.safeParse({ ...pet, homePosition: { x: Number.NaN, y: 80 } }).success).toBe(false);
  });

  it("accepts only bounded, sorted, unique known inventory stacks", () => {
    const pet = {
      id: "019bf2c6-4d40-7000-8000-000000000001",
      name: "Aster",
      state: "idle",
      emotion: "happy",
      position: { x: 0, y: 0 },
      energy: 80,
      mood: 70,
      affinity: 0,
    };
    expect(petSchema.safeParse({ ...pet, inventory: [{ itemId: "berry_bite", quantity: 1 }] }).success).toBe(true);
    expect(petSchema.safeParse({ ...pet, inventory: [{ itemId: "berry_bite", quantity: 0 }] }).success).toBe(false);
    expect(petSchema.safeParse({ ...pet, inventory: [{ itemId: "berry_bite", quantity: 1_000 }] }).success).toBe(false);
    expect(petSchema.safeParse({ ...pet, inventory: [{ itemId: "unknown", quantity: 1 }] }).success).toBe(false);
    expect(petSchema.safeParse({ ...pet, inventory: [
      { itemId: "star_ball", quantity: 1 },
      { itemId: "berry_bite", quantity: 1 },
    ] }).success).toBe(false);
    expect(petSchema.safeParse({ ...pet, inventory: [
      { itemId: "berry_bite", quantity: 1 },
      { itemId: "berry_bite", quantity: 2 },
    ] }).success).toBe(false);
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
          quietHours: { enabled: true, startMinute: 1320, endMinute: 420 },
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

  it("rejects invalid quiet-hour minutes", () => {
    const id = "019bf2c6-4d40-7000-8000-000000000010";
    expect(profileSnapshotSchema.safeParse({
      schemaVersion: 1,
      activeProfileId: id,
      profiles: [{
        id,
        name: "Night",
        policy: {
          mode: "companion",
          alwaysOnTop: true,
          clickThrough: false,
          soundEnabled: false,
          proactiveFrequency: 25,
          quietHours: { enabled: true, startMinute: 1440, endMinute: 420 },
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

  it("accepts only package-relative preview poster paths", () => {
    expect(assetManifestSchema.safeParse(validAssetManifest).success).toBe(true);
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      entrypoints: { ...validAssetManifest.entrypoints, previewPoster: "../poster.webp" },
    }).success).toBe(false);
  });

  it("requires clips exactly for sprite renderers", () => {
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      entrypoints: { animationGraph: "animations/graph.json" },
    }).success).toBe(false);
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      render: { ...validAssetManifest.render, backend: "vrm" },
    }).success).toBe(false);
  });

  it("requires model entrypoints exactly for model renderers", () => {
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      render: { ...validAssetManifest.render, backend: "gltf" },
      entrypoints: { model: "models/character.glb" },
    }).success).toBe(true);
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      render: { ...validAssetManifest.render, backend: "gltf" },
      entrypoints: {},
    }).success).toBe(false);
    expect(assetManifestSchema.safeParse({
      ...validAssetManifest,
      entrypoints: { ...validAssetManifest.entrypoints, model: "models/character.glb" },
    }).success).toBe(false);
  });
});

describe("spriteClipsSchema", () => {
  it("accepts bounded sequence and atlas documents", () => {
    expect(spriteClipsSchema.safeParse({
      spec: "nimora.sprite-clips/1",
      backend: "sprite-sequence",
      clips: { "pet.idle": { loop: true, frames: [{ file: "sprites/idle/0001.webp", durationMs: 100 }] } },
    }).success).toBe(true);
    expect(spriteClipsSchema.safeParse({
      spec: "nimora.sprite-clips/1",
      backend: "sprite-atlas",
      image: "sprites/atlas.webp",
      clips: { "pet.idle": { loop: true, frames: [{ x: 0, y: 0, width: 256, height: 256, durationMs: 100 }] } },
    }).success).toBe(true);
  });

  it("rejects path escape, missing idle, and unbounded timing", () => {
    expect(spriteClipsSchema.safeParse({
      spec: "nimora.sprite-clips/1",
      backend: "sprite-sequence",
      clips: { "pet.walk": { loop: true, frames: [{ file: "../escape.png", durationMs: 1 }] } },
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
