import { describe, expect, it, vi } from "vitest";
import { subscribeReducedMotion, type ReducedMotionPreference } from "./reducedMotion";

describe("subscribeReducedMotion", () => {
  it("publishes the initial value, changes, and removes its listener", () => {
    const listener: { current: (() => void) | null } = { current: null };
    const preference = {
      matches: false,
      addEventListener: vi.fn((_type: "change", next: () => void) => { listener.current = next; }),
      removeEventListener: vi.fn((_type: "change", next: () => void) => {
        if (listener.current === next) listener.current = null;
      }),
    } as ReducedMotionPreference;
    const publish = vi.fn();

    const dispose = subscribeReducedMotion(preference, publish);
    expect(publish).toHaveBeenLastCalledWith(false);

    Object.assign(preference, { matches: true });
    listener.current?.();
    expect(publish).toHaveBeenLastCalledWith(true);

    dispose();
    expect(listener.current).toBeNull();
  });
});
