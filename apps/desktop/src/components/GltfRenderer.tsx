import { useEffect, useRef } from "react";
import {
  AnimationAction,
  AnimationClip,
  AnimationMixer,
  Box3,
  Clock,
  DirectionalLight,
  HemisphereLight,
  LoopOnce,
  LoopRepeat,
  MathUtils,
  Mesh,
  Object3D,
  PerspectiveCamera,
  Scene,
  Sphere,
  SRGBColorSpace,
  Texture,
  Vector3,
  WebGLRenderer,
} from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { CharacterRendererSnapshot, ModelAnimationBinding } from "../platform/desktop";

export function modelAssetUrl(baseUrl: string, relativePath: string): string {
  const encoded = relativePath.split("/").map(encodeURIComponent).join("/");
  return `${baseUrl}${encoded}`;
}

export function cameraDistanceForRadius(radius: number, verticalFovDegrees: number): number {
  const safeRadius = Math.max(radius, 0.001);
  const halfFov = MathUtils.degToRad(verticalFovDegrees / 2);
  return safeRadius / Math.sin(halfFov);
}

export function disposeObjectTree(root: Object3D): void {
  root.traverse((object) => {
    if (!(object instanceof Mesh)) return;
    object.geometry?.dispose();
    const materials = Array.isArray(object.material) ? object.material : [object.material];
    for (const material of materials) {
      for (const value of Object.values(material)) {
        if (value instanceof Texture) value.dispose();
      }
      material.dispose();
    }
  });
}

interface GltfRendererProps {
  descriptor: CharacterRendererSnapshot;
  action: string;
  onFailure(): void;
}

export function resolveModelAnimation(
  action: string,
  clips: Record<string, ModelAnimationBinding>,
  fallbacks: Record<string, string>,
  available: ReadonlySet<string>,
): ModelAnimationBinding | null {
  let candidate = action;
  const visited = new Set<string>();
  while (!(candidate in clips) && !visited.has(candidate)) {
    visited.add(candidate);
    candidate = fallbacks[candidate] ?? "pet.idle";
  }
  const binding = clips[candidate] ?? clips["pet.idle"];
  return binding && available.has(binding.animation) ? binding : null;
}

export function GltfRenderer({ descriptor, action, onFailure }: GltfRendererProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const playActionRef = useRef<(action: string) => void>(() => undefined);
  const latestActionRef = useRef(action);

  useEffect(() => {
    latestActionRef.current = action;
    playActionRef.current(action);
  }, [action]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const baseUrl = descriptor.assetBaseUrl;
    const model = descriptor.model;
    if (!canvas || !baseUrl || !model || descriptor.backend !== "gltf") return;

    let disposed = false;
    let animationFrame = 0;
    let loadedRoot: Object3D | null = null;
    let mixer: AnimationMixer | null = null;
    let renderer: WebGLRenderer;
    try {
      renderer = new WebGLRenderer({ canvas, alpha: true, antialias: true });
    } catch {
      onFailure();
      return;
    }
    renderer.setClearColor(0x000000, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = SRGBColorSpace;

    const scene = new Scene();
    const camera = new PerspectiveCamera(35, 1, 0.01, 1000);
    scene.add(new HemisphereLight(0xfff5e8, 0x586270, 2.2));
    const keyLight = new DirectionalLight(0xffffff, 2.8);
    keyLight.position.set(3, 5, 4);
    scene.add(keyLight);

    const resize = () => {
      const width = Math.max(canvas.clientWidth, 1);
      const height = Math.max(canvas.clientHeight, 1);
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(canvas);
    resize();

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const clock = new Clock();
    const renderFrame = () => {
      if (disposed) return;
      animationFrame = window.requestAnimationFrame(renderFrame);
      const delta = clock.getDelta();
      if (!reducedMotion.matches) mixer?.update(delta);
      renderer.render(scene, camera);
    };
    animationFrame = window.requestAnimationFrame(renderFrame);

    const fail = () => {
      if (!disposed) onFailure();
    };
    const handleContextLost = (event: Event) => {
      event.preventDefault();
      fail();
    };
    canvas.addEventListener("webglcontextlost", handleContextLost);

    const loader = new GLTFLoader();
    loader.load(
      modelAssetUrl(baseUrl, model),
      (gltf) => {
        if (disposed) {
          disposeObjectTree(gltf.scene);
          return;
        }
        loadedRoot = gltf.scene;
        const bounds = new Box3().setFromObject(loadedRoot);
        if (bounds.isEmpty()) {
          fail();
          return;
        }
        const center = bounds.getCenter(new Vector3());
        const sphere = bounds.getBoundingSphere(new Sphere());
        loadedRoot.position.sub(center);
        loadedRoot.scale.multiplyScalar(descriptor.defaultScale);
        scene.add(loadedRoot);
        const radius = sphere.radius * descriptor.defaultScale;
        const distance = cameraDistanceForRadius(radius, camera.fov) * 1.18;
        camera.near = Math.max(distance / 100, 0.01);
        camera.far = Math.max(distance * 100, 100);
        camera.position.set(0, radius * 0.12, distance);
        camera.lookAt(0, 0, 0);
        camera.updateProjectionMatrix();
        const firstAnimation = gltf.animations[0];
        const animationMap = descriptor.animationMap;
        if (firstAnimation && animationMap) {
          mixer = new AnimationMixer(loadedRoot);
          const available = new Set(gltf.animations.map((clip) => clip.name));
          let current: AnimationAction | null = null;
          playActionRef.current = (nextAction) => {
            if (reducedMotion.matches || !mixer) return;
            const binding = resolveModelAnimation(
              nextAction,
              animationMap.clips,
              descriptor.fallbacks,
              available,
            );
            if (!binding) return;
            const clip = AnimationClip.findByName(gltf.animations, binding.animation);
            if (!clip) return;
            const next = mixer.clipAction(clip);
            if (next === current) return;
            next.reset();
            next.setLoop(binding.looped ? LoopRepeat : LoopOnce, binding.looped ? Infinity : 1);
            next.clampWhenFinished = !binding.looped;
            next.fadeIn(0.18).play();
            current?.fadeOut(0.18);
            current = next;
          };
          playActionRef.current(latestActionRef.current);
        }
      },
      undefined,
      fail,
    );

    return () => {
      disposed = true;
      window.cancelAnimationFrame(animationFrame);
      resizeObserver.disconnect();
      canvas.removeEventListener("webglcontextlost", handleContextLost);
      mixer?.stopAllAction();
      playActionRef.current = () => undefined;
      if (loadedRoot) {
        scene.remove(loadedRoot);
        disposeObjectTree(loadedRoot);
      }
      renderer.dispose();
      renderer.forceContextLoss();
    };
  }, [descriptor, onFailure]);

  return (
    <canvas
      ref={canvasRef}
      className="gltf-renderer"
      aria-hidden="true"
      style={{
        aspectRatio: `${descriptor.canvas.width} / ${descriptor.canvas.height}`,
        transformOrigin: `${descriptor.anchor.x * 100}% ${descriptor.anchor.y * 100}%`,
      }}
    />
  );
}
