export function isPetView(search: string): boolean {
  return new URLSearchParams(search).get("view") === "pet";
}

export function petBodyClasses(petView: boolean, nativeRuntime: boolean): string[] {
  if (!petView) return [];
  return nativeRuntime ? ["pet-window"] : ["pet-window", "pet-browser-preview"];
}
