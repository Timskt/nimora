/** Rolling-window lifeform render performance samples (pure, no DOM). */

/** Clamp extreme frame deltas so freezes don't poison the window forever. */
export const LIFEFORM_PERF_MIN_FRAME_MS = 0.5;
export const LIFEFORM_PERF_MAX_FRAME_MS = 250;

/** Host-reporting thresholds for idle render budget pressure. */
export const LIFEFORM_PERF_WARN_FPS = 45;
export const LIFEFORM_PERF_WARN_AVG_FRAME_MS = 22;

/** Window CustomEvent name for Control Center / host consumers. */
export const LIFEFORM_PERF_EVENT = "nimora:lifeform-perf";

/** Throttle interval for summary emit (not every rAF). */
export const LIFEFORM_PERF_EMIT_INTERVAL_MS = 500;

/** Product budgets for render + host process idle cost. */
export const LIFEFORM_PERF_BUDGET = {
  targetFps: 60,
  warnFps: LIFEFORM_PERF_WARN_FPS,
  dangerFps: 30,
  targetFrameMs: 1000 / 60,
  warnFrameMs: LIFEFORM_PERF_WARN_AVG_FRAME_MS,
  dangerFrameMs: 33,
  memoryTargetMb: 200,
  memoryWarnMb: 200,
  memoryDangerMb: 320,
  idleCpuTargetPercent: 2,
  idleCpuWarnPercent: 2,
  idleCpuDangerPercent: 10,
} as const;

export interface LifeformPerfSample {
  atMs: number;
  frameDtMs: number;
}

export interface LifeformPerfTracker {
  windowMs: number;
  samples: LifeformPerfSample[];
}

export interface LifeformPerfSummary {
  fps: number;
  avgFrameMs: number;
  maxFrameMs: number;
  sampleCount: number;
}

/** Optional host process budget (may land concurrently on lifeformSense). */
export interface LifeformHostProcessBudget {
  processRssMb?: number | null;
  processCpuPercent?: number | null;
  idleCpuPercent?: number | null;
}

export type LifeformPerfCardTone = "neutral" | "ok" | "warn" | "busy" | "danger";

/** Sense-card compatible model for Control Center 「渲染预算」. */
export interface LifeformPerfCard {
  id: string;
  label: string;
  value: string;
  detail?: string;
  tone: LifeformPerfCardTone;
}

export function clampFrameDtMs(frameDtMs: number): number {
  if (!Number.isFinite(frameDtMs)) return LIFEFORM_PERF_MIN_FRAME_MS;
  if (frameDtMs < LIFEFORM_PERF_MIN_FRAME_MS) return LIFEFORM_PERF_MIN_FRAME_MS;
  if (frameDtMs > LIFEFORM_PERF_MAX_FRAME_MS) return LIFEFORM_PERF_MAX_FRAME_MS;
  return frameDtMs;
}

export function createLifeformPerfTracker(windowMs = 1000): LifeformPerfTracker {
  const safeWindow = Number.isFinite(windowMs) && windowMs > 0 ? windowMs : 1000;
  return { windowMs: safeWindow, samples: [] };
}

function pruneWindow(tracker: LifeformPerfTracker, nowMs: number): void {
  const cutoff = nowMs - tracker.windowMs;
  const { samples } = tracker;
  let firstKeep = 0;
  while (firstKeep < samples.length && samples[firstKeep]!.atMs < cutoff) {
    firstKeep += 1;
  }
  if (firstKeep > 0) samples.splice(0, firstKeep);
}

/**
 * Record one rendered frame.
 * `nowMs` is an absolute clock (e.g. performance.now()); `frameDtMs` is the last frame delta.
 */
export function recordFrame(
  tracker: LifeformPerfTracker,
  nowMs: number,
  frameDtMs: number,
): void {
  if (!Number.isFinite(nowMs)) return;
  tracker.samples.push({ atMs: nowMs, frameDtMs: clampFrameDtMs(frameDtMs) });
  pruneWindow(tracker, nowMs);
}

export function summarize(tracker: LifeformPerfTracker): LifeformPerfSummary {
  const { samples } = tracker;
  const sampleCount = samples.length;
  if (sampleCount === 0) {
    return { fps: 0, avgFrameMs: 0, maxFrameMs: 0, sampleCount: 0 };
  }

  let totalDt = 0;
  let maxFrameMs = 0;
  for (const sample of samples) {
    totalDt += sample.frameDtMs;
    if (sample.frameDtMs > maxFrameMs) maxFrameMs = sample.frameDtMs;
  }
  const avgFrameMs = totalDt / sampleCount;

  let fps = 0;
  if (sampleCount >= 2) {
    const spanMs = samples[sampleCount - 1]!.atMs - samples[0]!.atMs;
    if (spanMs > 0) fps = ((sampleCount - 1) / spanMs) * 1000;
  } else if (avgFrameMs > 0) {
    fps = 1000 / avgFrameMs;
  }

  return { fps, avgFrameMs, maxFrameMs, sampleCount };
}

/** Pure flag for future host reporting when idle render budget is under pressure. */
export function shouldWarnIdleBudget(summary: LifeformPerfSummary): boolean {
  if (summary.sampleCount === 0) return false;
  return summary.fps < LIFEFORM_PERF_WARN_FPS || summary.avgFrameMs > LIFEFORM_PERF_WARN_AVG_FRAME_MS;
}

/** Gate for ~500ms summary emit from the render loop. */
export function createPerfEmitGate(intervalMs = LIFEFORM_PERF_EMIT_INTERVAL_MS): {
  shouldEmit: (nowMs: number) => boolean;
  reset: () => void;
} {
  const safeInterval =
    Number.isFinite(intervalMs) && intervalMs > 0 ? intervalMs : LIFEFORM_PERF_EMIT_INTERVAL_MS;
  let lastEmitMs = Number.NEGATIVE_INFINITY;
  return {
    shouldEmit(nowMs: number): boolean {
      if (!Number.isFinite(nowMs)) return false;
      if (nowMs - lastEmitMs < safeInterval) return false;
      lastEmitMs = nowMs;
      return true;
    },
    reset(): void {
      lastEmitMs = Number.NEGATIVE_INFINITY;
    },
  };
}

export function formatFps(summary: Pick<LifeformPerfSummary, "fps" | "sampleCount">): string {
  if (summary.sampleCount <= 0) return "采样中";
  if (!Number.isFinite(summary.fps) || summary.fps <= 0) return "—";
  const rounded = summary.fps >= 10 ? Math.round(summary.fps) : Math.round(summary.fps * 10) / 10;
  return `${rounded} 帧/秒`;
}

export function formatFrameMs(frameMs: number, digits = 1): string {
  if (!Number.isFinite(frameMs) || frameMs <= 0) return "—";
  const fixed = frameMs >= 100 ? Math.round(frameMs) : Number(frameMs.toFixed(digits));
  return `${fixed} ms`;
}

export function formatMemoryMb(mb: number): string {
  if (!Number.isFinite(mb) || mb < 0) return "—";
  const value = mb >= 100 ? Math.round(mb) : Math.round(mb * 10) / 10;
  return `${value} MB`;
}

export function formatCpuPercent(percent: number): string {
  if (!Number.isFinite(percent) || percent < 0) return "—";
  const value = percent >= 10 ? Math.round(percent) : Math.round(percent * 10) / 10;
  return `${value}%`;
}

function fpsTone(fps: number, idleWarn: boolean): LifeformPerfCardTone {
  if (!Number.isFinite(fps) || fps <= 0) return "neutral";
  if (fps < LIFEFORM_PERF_BUDGET.dangerFps) return "danger";
  if (fps < LIFEFORM_PERF_BUDGET.warnFps || idleWarn) return "warn";
  return "ok";
}

function frameTone(avgFrameMs: number, idleWarn: boolean): LifeformPerfCardTone {
  if (!Number.isFinite(avgFrameMs) || avgFrameMs <= 0) return "neutral";
  if (avgFrameMs >= LIFEFORM_PERF_BUDGET.dangerFrameMs) return "danger";
  if (avgFrameMs > LIFEFORM_PERF_BUDGET.warnFrameMs || idleWarn) return "warn";
  return "ok";
}

function memoryTone(mb: number): LifeformPerfCardTone {
  if (!Number.isFinite(mb) || mb < 0) return "neutral";
  if (mb >= LIFEFORM_PERF_BUDGET.memoryDangerMb) return "danger";
  if (mb >= LIFEFORM_PERF_BUDGET.memoryWarnMb) return "warn";
  return "ok";
}

function cpuTone(percent: number): LifeformPerfCardTone {
  if (!Number.isFinite(percent) || percent < 0) return "neutral";
  if (percent >= LIFEFORM_PERF_BUDGET.idleCpuDangerPercent) return "danger";
  if (percent >= LIFEFORM_PERF_BUDGET.idleCpuWarnPercent) return "warn";
  return "ok";
}

/**
 * Defensively project optional host process fields from a loose sense snapshot.
 * Accepts `processRssMb` / `processCpuPercent` or nested `processBudget`.
 */
export function hostProcessBudgetFromSense(sense: unknown): LifeformHostProcessBudget | null {
  if (!sense || typeof sense !== "object") return null;
  const root = sense as Record<string, unknown>;
  const nested =
    root.processBudget && typeof root.processBudget === "object"
      ? (root.processBudget as Record<string, unknown>)
      : null;

  const processRssMb = pickFiniteNumber(
    root.processRssMb,
    nested?.processRssMb,
    nested?.rssMb,
    nested?.memoryMb,
  );
  const processCpuPercent = pickFiniteNumber(
    root.processCpuPercent,
    nested?.processCpuPercent,
    nested?.cpuPercent,
  );
  const idleCpuPercent = pickFiniteNumber(
    root.idleCpuPercent,
    nested?.idleCpuPercent,
  );

  if (processRssMb == null && processCpuPercent == null && idleCpuPercent == null) {
    return null;
  }

  const budget: LifeformHostProcessBudget = {};
  if (processRssMb != null) budget.processRssMb = processRssMb;
  if (processCpuPercent != null) budget.processCpuPercent = processCpuPercent;
  if (idleCpuPercent != null) budget.idleCpuPercent = idleCpuPercent;
  return budget;
}

function pickFiniteNumber(...values: unknown[]): number | null {
  for (const value of values) {
    if (typeof value === "number" && Number.isFinite(value) && value >= 0) return value;
  }
  return null;
}

/**
 * Build Control Center cards for 「渲染预算」.
 * Skips empty render summary; includes optional host memory / idle CPU when present.
 */
export function buildPerfBudgetCards(
  summary?: LifeformPerfSummary | null,
  hostBudget?: LifeformHostProcessBudget | null,
): LifeformPerfCard[] {
  const cards: LifeformPerfCard[] = [];
  const idleWarn = summary ? shouldWarnIdleBudget(summary) : false;

  if (summary && summary.sampleCount > 0) {
    cards.push({
      id: "fps",
      label: "帧率",
      value: formatFps(summary),
      detail: `目标 ${LIFEFORM_PERF_BUDGET.targetFps} 帧/秒`,
      tone: fpsTone(summary.fps, idleWarn),
    });

    const frameCard: LifeformPerfCard = {
      id: "frame",
      label: "帧耗时",
      value: formatFrameMs(summary.avgFrameMs),
      detail: `峰值 ${formatFrameMs(summary.maxFrameMs)} · 告警 >${LIFEFORM_PERF_BUDGET.warnFrameMs} ms`,
      tone: frameTone(summary.avgFrameMs, idleWarn),
    };
    cards.push(frameCard);

    cards.push({
      id: "idle-budget",
      label: "空闲预算",
      value: idleWarn ? "偏紧" : "充裕",
      detail: idleWarn
        ? "渲染占用偏高，注意卡顿"
        : `目标 ${LIFEFORM_PERF_BUDGET.targetFps} fps · 空闲 CPU <${LIFEFORM_PERF_BUDGET.idleCpuTargetPercent}%`,
      tone: idleWarn ? "warn" : "ok",
    });
  }

  const rss = hostBudget?.processRssMb;
  if (typeof rss === "number" && Number.isFinite(rss) && rss >= 0) {
    cards.push({
      id: "memory",
      label: "内存",
      value: formatMemoryMb(rss),
      detail: `目标 <${LIFEFORM_PERF_BUDGET.memoryTargetMb} MB`,
      tone: memoryTone(rss),
    });
  }

  const cpu =
    typeof hostBudget?.idleCpuPercent === "number" && Number.isFinite(hostBudget.idleCpuPercent)
      ? hostBudget.idleCpuPercent
      : typeof hostBudget?.processCpuPercent === "number" && Number.isFinite(hostBudget.processCpuPercent)
        ? hostBudget.processCpuPercent
        : null;
  if (cpu != null && cpu >= 0) {
    cards.push({
      id: "cpu",
      label: "空闲 CPU",
      value: formatCpuPercent(cpu),
      detail: `目标 <${LIFEFORM_PERF_BUDGET.idleCpuTargetPercent}%`,
      tone: cpuTone(cpu),
    });
  }

  return cards;
}
