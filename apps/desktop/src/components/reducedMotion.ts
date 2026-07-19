export type ReducedMotionPreference = Pick<MediaQueryList, "matches" | "addEventListener" | "removeEventListener">;

export function subscribeReducedMotion(
  preference: ReducedMotionPreference,
  publish: (enabled: boolean) => void,
): () => void {
  const synchronize = () => publish(preference.matches);
  synchronize();
  preference.addEventListener("change", synchronize);
  return () => preference.removeEventListener("change", synchronize);
}
