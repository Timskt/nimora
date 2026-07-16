import { z } from "zod";

const namespacedId = z
  .string()
  .regex(/^[a-z0-9-]+(?:\.[a-z0-9-]+){2,}$/, "must be a lowercase namespaced identifier");

export const eventSourceSchema = z.union([
  z.literal("core"),
  z.string().regex(/^(skill|automation|agent|connector|gateway|system):[^:]+$/),
]);

export const eventSchema = z.object({
  spec: z.literal("nimora.event/1"),
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
  spec: z.literal("nimora.command/1"),
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

export const pointerButtonSchema = z.enum(["left", "middle", "right"]);

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

export const profilePolicySchema = z.object({
  alwaysOnTop: z.boolean().nullable(),
  clickThrough: z.boolean().nullable(),
  soundEnabled: z.boolean().nullable(),
  proactiveFrequency: z.number().int().min(0).max(100).nullable(),
});

export const profileSchema = z.object({
  id: z.uuid(),
  name: z.string().trim().min(1).max(64),
  policy: profilePolicySchema,
});

export const profileSnapshotSchema = z.object({
  schemaVersion: z.literal(1),
  activeProfileId: z.uuid(),
  profiles: z.array(profileSchema).min(1),
});

export const runtimeModeSchema = z.enum(["normal", "safe"]);
export const safeModeReasonSchema = z.enum([
  "manual",
  "crash_loop",
  "data_recovery",
  "policy_violation",
]);
export const safetySnapshotSchema = z.object({
  mode: runtimeModeSchema,
  reason: safeModeReasonSchema.nullable(),
}).superRefine((snapshot, context) => {
  if ((snapshot.mode === "safe") !== (snapshot.reason !== null)) {
    context.addIssue({
      code: "custom",
      message: "safe mode requires a reason and normal mode forbids one",
      path: ["reason"],
    });
  }
});

export type NimoraEvent = z.infer<typeof eventSchema>;
export type NimoraCommand = z.infer<typeof commandSchema>;
export type Pet = z.infer<typeof petSchema>;
export type PointerButton = z.infer<typeof pointerButtonSchema>;
export type ProfilePolicy = z.infer<typeof profilePolicySchema>;
export type Profile = z.infer<typeof profileSchema>;
export type ProfileSnapshot = z.infer<typeof profileSnapshotSchema>;
export type RuntimeMode = z.infer<typeof runtimeModeSchema>;
export type SafeModeReason = z.infer<typeof safeModeReasonSchema>;
export type SafetySnapshot = z.infer<typeof safetySnapshotSchema>;
