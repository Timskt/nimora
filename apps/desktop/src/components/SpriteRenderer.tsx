import { useEffect, useRef, useState } from "react";
import type { SpriteClips } from "@nimora/schemas";
import type { CharacterRendererSnapshot } from "../platform/desktop";

type SequenceDocument = Extract<SpriteClips, { backend: "sprite-sequence" }>;
type AtlasDocument = Extract<SpriteClips, { backend: "sprite-atlas" }>;

export function petStateAction(state: string): string {
  return ({
    idle: "pet.idle",
    observing: "pet.observe",
    walking: "pet.walk",
    sleeping: "pet.sleep",
    dragged: "pet.drag",
    interacting: "pet.click",
    working: "pet.work",
    recovering: "pet.idle",
  } as Record<string, string>)[state] ?? "pet.idle";
}

export function resolveSpriteAction(
  requested: string,
  clips: SpriteClips["clips"],
  fallbacks: Record<string, string>,
): string {
  let action = requested;
  const visited = new Set<string>();
  while (!(action in clips) && !visited.has(action)) {
    visited.add(action);
    action = fallbacks[action] ?? "pet.idle";
  }
  return action in clips ? action : "pet.idle";
}

export function assetImageUrl(baseUrl: string, relativePath: string): string {
  const encoded = relativePath.split("/").map(encodeURIComponent).join("/");
  return `${baseUrl}${encoded}`;
}

export function nextFrameIndex(current: number, length: number, loop: boolean): number {
  if (length <= 1 || current < 0 || current >= length) return 0;
  if (current + 1 < length) return current + 1;
  return loop ? 0 : current;
}

function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return reduced;
}

function useFrameIndex(frames: readonly { durationMs: number }[], loop: boolean, reduced: boolean): number {
  const [frameIndex, setFrameIndex] = useState(0);
  useEffect(() => {
    setFrameIndex(0);
  }, [frames]);
  useEffect(() => {
    if (reduced || frames.length <= 1) return;
    const current = frames[frameIndex];
    if (!current) return;
    if (!loop && frameIndex === frames.length - 1) return;
    const timer = window.setTimeout(() => {
      setFrameIndex((value) => nextFrameIndex(value, frames.length, loop));
    }, current.durationMs);
    return () => window.clearTimeout(timer);
  }, [frameIndex, frames, loop, reduced]);
  return frameIndex;
}

interface SpriteRendererProps {
  descriptor: CharacterRendererSnapshot;
  action: string;
  onFailure(): void;
}

export function SpriteRenderer({ descriptor, action, onFailure }: SpriteRendererProps) {
  const reduced = useReducedMotion();
  const clips = descriptor.clips;
  const baseUrl = descriptor.assetBaseUrl;
  if (!clips || !baseUrl || descriptor.backend === "built-in") return null;
  const resolved = resolveSpriteAction(action, clips.clips, descriptor.fallbacks);
  const style = {
    aspectRatio: `${descriptor.canvas.width} / ${descriptor.canvas.height}`,
    imageRendering: descriptor.pixelArt ? "pixelated" : "auto",
    transformOrigin: `${descriptor.anchor.x * 100}% ${descriptor.anchor.y * 100}%`,
    "--sprite-scale": String(descriptor.defaultScale),
  } as React.CSSProperties;
  if (clips.backend === "sprite-sequence") {
    return <SequenceRenderer document={clips} action={resolved} baseUrl={baseUrl} reduced={reduced} style={style} onFailure={onFailure} />;
  }
  return <AtlasRenderer document={clips} action={resolved} baseUrl={baseUrl} reduced={reduced} style={style} onFailure={onFailure} />;
}

function SequenceRenderer({ document, action, baseUrl, reduced, style, onFailure }: {
  document: SequenceDocument;
  action: string;
  baseUrl: string;
  reduced: boolean;
  style: React.CSSProperties;
  onFailure(): void;
}) {
  const clip = (document.clips[action] ?? document.clips["pet.idle"])!;
  const frameIndex = useFrameIndex(clip.frames, clip.loop, reduced);
  const frame = (clip.frames[frameIndex] ?? clip.frames[0])!;
  return <img className="sprite-renderer" src={assetImageUrl(baseUrl, frame.file)} alt="" draggable={false} style={style} onError={onFailure} />;
}

function AtlasRenderer({ document, action, baseUrl, reduced, style, onFailure }: {
  document: AtlasDocument;
  action: string;
  baseUrl: string;
  reduced: boolean;
  style: React.CSSProperties;
  onFailure(): void;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const clip = (document.clips[action] ?? document.clips["pet.idle"])!;
  const frameIndex = useFrameIndex(clip.frames, clip.loop, reduced);
  const frame = (clip.frames[frameIndex] ?? clip.frames[0])!;
  useEffect(() => {
    const target = canvas.current;
    if (!target) return;
    let cancelled = false;
    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      if (cancelled) return;
      if (frame.x + frame.width > image.naturalWidth || frame.y + frame.height > image.naturalHeight) {
        onFailure();
        return;
      }
      target.width = frame.width;
      target.height = frame.height;
      const context = target.getContext("2d");
      if (!context) {
        onFailure();
        return;
      }
      context.clearRect(0, 0, frame.width, frame.height);
      context.imageSmoothingEnabled = !style.imageRendering || style.imageRendering !== "pixelated";
      context.drawImage(image, frame.x, frame.y, frame.width, frame.height, 0, 0, frame.width, frame.height);
    };
    image.onerror = onFailure;
    image.src = assetImageUrl(baseUrl, document.image);
    return () => {
      cancelled = true;
      image.onload = null;
      image.onerror = null;
    };
  }, [baseUrl, document.image, frame, onFailure, style.imageRendering]);
  return <canvas ref={canvas} className="sprite-renderer" aria-hidden="true" style={style} />;
}
