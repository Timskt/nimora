import {
  Box3,
  CanvasTexture,
  MathUtils,
  Object3D,
  Sphere,
  SRGBColorSpace,
  Vector3,
} from "three";

/** Radial stops for grounded contact shadows (center → edge). */
export const CONTACT_SHADOW_STOPS: ReadonlyArray<readonly [number, string]> = [
  [0, "rgba(12, 14, 20, 0.9)"],
  [0.1, "rgba(12, 14, 20, 0.68)"],
  [0.24, "rgba(12, 14, 20, 0.4)"],
  [0.46, "rgba(12, 14, 20, 0.16)"],
  [0.7, "rgba(12, 14, 20, 0.05)"],
  [0.88, "rgba(12, 14, 20, 0.012)"],
  [1, "rgba(12, 14, 20, 0)"],
];

export function cameraDistanceForRadius(radius: number, verticalFovDegrees: number): number {
  const safeRadius = Math.max(radius, 0.001);
  const halfFov = MathUtils.degToRad(verticalFovDegrees / 2);
  return safeRadius / Math.sin(halfFov);
}

export function frameGroundedModel(root: Object3D, scale: number): {
  height: number;
  radius: number;
  lookAtY: number;
  spanX: number;
  spanZ: number;
} {
  root.scale.multiplyScalar(scale);
  root.updateMatrixWorld(true);
  const bounds = new Box3().setFromObject(root);
  if (bounds.isEmpty()) {
    return { height: 1, radius: 1, lookAtY: 0.45, spanX: 1, spanZ: 1 };
  }
  const center = bounds.getCenter(new Vector3());
  const size = bounds.getSize(new Vector3());
  root.position.x -= center.x;
  root.position.z -= center.z;
  root.position.y -= bounds.min.y;
  root.updateMatrixWorld(true);
  const grounded = new Box3().setFromObject(root);
  const sphere = grounded.getBoundingSphere(new Sphere());
  const height = Math.max(grounded.max.y - grounded.min.y, 0.001);
  return {
    height,
    radius: Math.max(sphere.radius, 0.001),
    lookAtY: height * 0.42,
    spanX: Math.max(size.x, 0.001),
    spanZ: Math.max(size.z, 0.001),
  };
}

export function cameraDistanceForGroundedPet(
  height: number,
  spanX: number,
  spanZ: number,
  verticalFovDegrees: number,
  fill = 0.78,
): number {
  const vertical = cameraDistanceForRadius((height * fill) / 2, verticalFovDegrees);
  const bodySpan = Math.max(spanX, Math.min(spanZ, height * 1.1));
  const horizontal = cameraDistanceForRadius((bodySpan * fill) / 2, verticalFovDegrees);
  return Math.max(vertical, horizontal) * 1.02;
}

/** Slightly oversized soft oval under grounded feet. */
export function contactShadowRadius(spanX: number, spanZ: number): number {
  return Math.max(spanX, spanZ * 0.38) * 0.68;
}

export function createContactShadowTexture(size = 128): CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    const empty = new CanvasTexture(canvas);
    empty.colorSpace = SRGBColorSpace;
    return empty;
  }
  const gradient = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  for (const [offset, color] of CONTACT_SHADOW_STOPS) {
    gradient.addColorStop(offset, color);
  }
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, size, size);
  const texture = new CanvasTexture(canvas);
  texture.colorSpace = SRGBColorSpace;
  texture.needsUpdate = true;
  return texture;
}
