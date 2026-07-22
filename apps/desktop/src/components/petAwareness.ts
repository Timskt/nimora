/**
 * Environmental awareness for the desktop lifeform.
 *
 * Milestone 1 ("sensory awakening") asks the pet to react to what is happening
 * on the desktop. This repo's security model is deliberate and load-bearing:
 * sensors expose only *boolean facts* (is the foreground fullscreen? is Do Not
 * Disturb on? is a game running?) and never window titles, rectangles, z-order,
 * or screen pixels. This module honors that boundary — it maps the privacy-safe
 * system-context the host already publishes to a small reaction the pet can
 * perform, so the creature feels aware of its environment without ever learning
 * *what* the user is doing.
 *
 * It is a pure, DOM-free mapping: given the current presence decision it returns
 * a reaction (an expression cue and whether the pet should quiet down). No React,
 * no window, no IPC — it unit-tests in isolation and stays inside the
 * architecture boundary.
 */

/**
 * The privacy-safe reasons the host may give for the pet's current presence
 * decision. Mirrors `PresenceDecisionReason` in the platform adapter; carries no
 * window or content information — only which boolean environment fact is in
 * effect.
 */
export type AwarenessReason =
  | "base_policy"
  | "user_forced_visible"
  | "user_forced_hidden"
  | "safe_mode_recovery"
  | "do_not_disturb"
  | "fullscreen"
  | "game"
  | "screen_share_privacy";

/** How the pet expresses its read of the environment. */
export type AwarenessMood = "neutral" | "calm" | "focused" | "playful" | "shy";

/** The pet's reaction to the current environment. */
export interface AwarenessReaction {
  /** Expression cue the character can adopt. */
  mood: AwarenessMood;
  /**
   * Whether the pet should quiet down (settle, stop soliciting attention). True
   * whenever the environment signals the user is busy or wants no interruption.
   */
  quiet: boolean;
  /**
   * A short, content-free status line. Deliberately generic — it reflects the
   * boolean fact only, never what the user is actually doing.
   */
  cue: string;
}

/** The pet's default, unremarkable reaction to a plain desktop. */
export const NEUTRAL_AWARENESS: AwarenessReaction = {
  mood: "neutral",
  quiet: false,
  cue: "本地陪伴中",
};

/**
 * Maps the current presence-decision reason to the pet's reaction.
 *
 * The mapping is intentionally coarse — it only knows the single boolean fact
 * the host chose to surface:
 * - `fullscreen` / `game`: the user is immersed → the pet quiets and watches
 *   calmly rather than soliciting attention.
 * - `do_not_disturb`: the user asked not to be disturbed → the pet goes quiet
 *   and shy.
 * - `screen_share_privacy`: something is being shared → the pet stays calm and
 *   out of the way.
 * - `user_forced_hidden` / `safe_mode_recovery`: the pet is suppressed → neutral
 *   and quiet.
 * - everything else: ordinary companionship.
 *
 * An unknown reason falls back to {@link NEUTRAL_AWARENESS} rather than throwing,
 * so a future host reason can never break the renderer.
 */
export function awarenessFromReason(reason: string): AwarenessReaction {
  switch (reason) {
    case "fullscreen":
      return { mood: "focused", quiet: true, cue: "你在专注，我安静看着" };
    case "game":
      return { mood: "playful", quiet: true, cue: "在玩游戏呀，我不打扰" };
    case "do_not_disturb":
      return { mood: "shy", quiet: true, cue: "免打扰中，我小声待着" };
    case "screen_share_privacy":
      return { mood: "calm", quiet: true, cue: "共享画面时我先躲一躲" };
    case "user_forced_hidden":
    case "safe_mode_recovery":
      return { mood: "neutral", quiet: true, cue: NEUTRAL_AWARENESS.cue };
    case "base_policy":
    case "user_forced_visible":
      return NEUTRAL_AWARENESS;
    default:
      return NEUTRAL_AWARENESS;
  }
}

/**
 * Reports whether an awareness reason represents a "busy" environment the pet
 * should defer to. Useful for callers that only need the boolean, not the full
 * reaction (e.g. deciding whether to suppress an idle animation).
 */
export function isBusyEnvironment(reason: string): boolean {
  return awarenessFromReason(reason).quiet;
}
