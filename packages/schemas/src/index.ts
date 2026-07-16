import { z } from "zod";

const namespacedId = z
  .string()
  .regex(/^[a-z0-9-]+(?:\.[a-z0-9-]+){2,}$/, "must be a lowercase namespaced identifier");

export const eventSourceSchema = z.union([
  z.literal("core"),
  z.string().regex(/^(skill|automation|agent|connector|gateway|system):[^:]+$/),
]);

export const eventSchema = z.object({
  spec: z.literal("asterpet.event/1"),
  id: z.uuid(),
  eventType: namespacedId,
  source: eventSourceSchema,
  timestamp: z.iso.datetime({ offset: true }),
  traceId: z.uuid(),
  data: z.unknown(),
});

export const commandRiskSchema = z.enum(["safe", "low", "medium", "high", "critical"]);
export const commandStatusSchema = z.enum([
  "pending",
  "running",
  "succeeded",
  "failed",
  "cancelled",
  "timed_out",
]);

export const commandSchema = z.object({
  spec: z.literal("asterpet.command/1"),
  executionId: z.uuid(),
  commandId: namespacedId,
  traceId: z.uuid(),
  arguments: z.unknown(),
  risk: commandRiskSchema,
  status: commandStatusSchema,
  idempotencyKey: z.string().min(1).nullable(),
});

export const petStateSchema = z.enum([
  "idle",
  "walking",
  "sleeping",
  "dragged",
  "interacting",
  "working",
  "recovering",
]);

export const emotionSchema = z.enum([
  "neutral",
  "happy",
  "sad",
  "angry",
  "surprised",
  "focused",
  "sleepy",
]);

export const petSchema = z.object({
  id: z.uuid(),
  name: z.string().trim().min(1).max(64),
  state: petStateSchema,
  emotion: emotionSchema,
  position: z.object({ x: z.number().finite(), y: z.number().finite() }),
  energy: z.number().int().min(0).max(100),
  mood: z.number().int().min(0).max(100),
  affinity: z.number().int().min(0).max(100),
});

export type AsterEvent = z.infer<typeof eventSchema>;
export type AsterCommand = z.infer<typeof commandSchema>;
export type Pet = z.infer<typeof petSchema>;

