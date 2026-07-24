import { useEffect, useRef } from "react";
import {
  AnimationAction,
  AnimationClip,
  AnimationMixer,
  Box3,
  CanvasTexture,
  CircleGeometry,
  DirectionalLight,
  Group,
  HemisphereLight,
  LoopOnce,
  LoopRepeat,
  MathUtils,
  Mesh,
  MeshBasicMaterial,
  Object3D,
  PerspectiveCamera,
  Scene,
  SRGBColorSpace,
  Texture,
  Timer,
  WebGLRenderer,
} from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { VRM } from "@pixiv/three-vrm";
import type { CharacterRendererSnapshot, ModelAnimationBinding } from "../platform/desktop";
import {
  cameraDistanceForGroundedPet,
  contactShadowRadius,
  createContactShadowTexture,
  frameGroundedModel,
} from "./petSceneHelpers";
import { applyVrmExpression, type VrmExpressionController } from "./vrmExpressions";

export {
  cameraDistanceForGroundedPet,
  cameraDistanceForRadius,
  contactShadowRadius,
  createContactShadowTexture,
  frameGroundedModel,
} from "./petSceneHelpers";

export function modelAssetUrl(baseUrl: string, relativePath: string): string {
  const encoded = relativePath.split("/").map(encodeURIComponent).join("/");
  return `${baseUrl}${encoded}`;
}

export function isThreeDimensionalBackend(backend: string): boolean {
  return backend === "gltf" || backend === "vrm";
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

export function dispatchModelAction(
  backend: string,
  expressionController: VrmExpressionController | null | undefined,
  playAnimation: ((action: string) => void) | null,
  action: string,
  expressionOverrides?: Parameters<typeof applyVrmExpression>[2],
): void {
  if (backend === "vrm") applyVrmExpression(expressionController, action, expressionOverrides);
  playAnimation?.(action);
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
    if (!canvas || !baseUrl || !model || !isThreeDimensionalBackend(descriptor.backend)) return;

    let disposed = false;
    let animationFrame = 0;
    let loadedRoot: Object3D | null = null;
    let mixer: AnimationMixer | null = null;
    let vrm: VRM | null = null;
    let disposeVrm: ((value: VRM) => void) | null = null;
    let shadowTexture: CanvasTexture | null = null;
    let contactShadow: Mesh | null = null;
    let petStage: Group | null = null;
    let modelRoot: Group | null = null;
    let renderer: WebGLRenderer;
    try {
      renderer = new WebGLRenderer({
        canvas,
        alpha: true,
        antialias: true,
        premultipliedAlpha: true,
        powerPreference: "high-performance",
      });
    } catch {
      onFailure();
      return;
    }
    renderer.setClearColor(0x000000, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = SRGBColorSpace;

    const scene = new Scene();
    const camera = new PerspectiveCamera(32, 1, 0.01, 1000);
    scene.add(new HemisphereLight(0xfff4e8, 0x4a5568, 1.55));
    const keyLight = new DirectionalLight(0xfff7ee, 2.35);
    keyLight.position.set(2.4, 5.2, 3.6);
    scene.add(keyLight);
    const fillLight = new DirectionalLight(0xc8d8f0, 0.85);
    fillLight.position.set(-3.2, 2.4, -1.6);
    scene.add(fillLight);
    const rimLight = new DirectionalLight(0xffe0c4, 0.55);
    rimLight.position.set(-1.2, 3.5, -4.2);
    scene.add(rimLight);

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
    const timer = new Timer();
    let lookAtY = 0.4;
    let breathAmplitude = 0;
    let gazeX = 0;
    let gazeY = 0;
    const trackPointer = (event: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const midX = rect.left + rect.width / 2;
      const midY = rect.top + rect.height / 2;
      gazeX = MathUtils.clamp((event.clientX - midX) / Math.max(rect.width * 0.55, 1), -1, 1);
      gazeY = MathUtils.clamp((event.clientY - midY) / Math.max(rect.height * 0.55, 1), -1, 1);
    };
    window.addEventListener("pointermove", trackPointer, { passive: true });

    const renderFrame = () => {
      if (disposed) return;
      animationFrame = window.requestAnimationFrame(renderFrame);
      timer.update();
      const delta = timer.getDelta();
      const elapsed = timer.getElapsed();
      if (!reducedMotion.matches) {
        mixer?.update(delta);
        vrm?.update(delta);
        if (modelRoot) {
          const breath = Math.sin(elapsed * 1.55) * breathAmplitude;
          modelRoot.position.y = breath;
          modelRoot.rotation.y = MathUtils.lerp(modelRoot.rotation.y, gazeX * 0.12, 0.06);
          modelRoot.rotation.x = MathUtils.lerp(modelRoot.rotation.x, gazeY * 0.04, 0.06);
        }
        if (contactShadow) {
          const pulse = 1 + Math.sin(elapsed * 1.55) * 0.045;
          contactShadow.scale.x = pulse;
          contactShadow.scale.z = pulse * 0.58;
          const material = contactShadow.material as MeshBasicMaterial;
          material.opacity = 0.98 - Math.sin(elapsed * 1.55) * 0.08;
        }
      }
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

    const loadModel = async () => {
      const loader = new GLTFLoader();
      if (descriptor.backend === "vrm") {
        const { VRMLoaderPlugin, VRMUtils } = await import("@pixiv/three-vrm");
        if (disposed) return;
        loader.register((parser) => new VRMLoaderPlugin(parser));
        disposeVrm = (value) => VRMUtils.deepDispose(value.scene);
      }
      loader.load(
      modelAssetUrl(baseUrl, model),
      (gltf) => {
        if (disposed) {
          disposeObjectTree(gltf.scene);
          return;
        }
        loadedRoot = gltf.scene;
        if (descriptor.backend === "vrm") {
          vrm = gltf.userData.vrm as VRM | undefined ?? null;
          if (!vrm) {
            fail();
            disposeObjectTree(gltf.scene);
            return;
          }
        }
        const bounds = new Box3().setFromObject(loadedRoot);
        if (bounds.isEmpty()) {
          fail();
          return;
        }

        petStage = new Group();
        modelRoot = new Group();
        modelRoot.add(loadedRoot);
        const framed = frameGroundedModel(loadedRoot, descriptor.defaultScale);
        lookAtY = framed.lookAtY;
        breathAmplitude = framed.height * 0.01;

        shadowTexture = createContactShadowTexture();
        const shadowRadius = contactShadowRadius(framed.spanX, framed.spanZ);
        const shadowGeo = new CircleGeometry(shadowRadius, 48);
        const shadowMat = new MeshBasicMaterial({
          map: shadowTexture,
          transparent: true,
          opacity: 1,
          depthWrite: false,
        });
        contactShadow = new Mesh(shadowGeo, shadowMat);
        contactShadow.rotation.x = -Math.PI / 2;
        contactShadow.position.y = 0.002;
        contactShadow.scale.set(1, 1, 0.58);
        petStage.add(contactShadow);
        petStage.add(modelRoot);
        scene.add(petStage);

        const distance = cameraDistanceForGroundedPet(
          framed.height,
          framed.spanX,
          framed.spanZ,
          camera.fov,
        );
        camera.near = Math.max(distance / 100, 0.01);
        camera.far = Math.max(distance * 100, 100);
        camera.position.set(0, lookAtY * 1.06, distance);
        camera.lookAt(0, lookAtY, 0);
        camera.updateProjectionMatrix();

        const animationMap = descriptor.animationMap;
        let playModelAnimation: ((nextAction: string) => void) | null = null;
        if (gltf.animations.length > 0 && animationMap) {
          mixer = new AnimationMixer(loadedRoot);
          const available = new Set(gltf.animations.map((clip) => clip.name));
          let current: AnimationAction | null = null;
          playModelAnimation = (nextAction) => {
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
        }
        playActionRef.current = (nextAction) => {
          dispatchModelAction(
            descriptor.backend,
            vrm?.expressionManager,
            playModelAnimation,
            nextAction,
            descriptor.vrmExpressionMap?.expressions,
          );
        };
        playActionRef.current(latestActionRef.current);
      },
      undefined,
      fail,
      );
    };
    void loadModel().catch(fail);

    return () => {
      disposed = true;
      timer.dispose();
      window.cancelAnimationFrame(animationFrame);
      window.removeEventListener("pointermove", trackPointer);
      resizeObserver.disconnect();
      canvas.removeEventListener("webglcontextlost", handleContextLost);
      mixer?.stopAllAction();
      vrm?.expressionManager?.resetValues();
      playActionRef.current = () => undefined;
      if (petStage) scene.remove(petStage);
      if (contactShadow) {
        contactShadow.geometry.dispose();
        (contactShadow.material as MeshBasicMaterial).dispose();
      }
      shadowTexture?.dispose();
      if (loadedRoot) {
        if (vrm && disposeVrm) disposeVrm(vrm);
        else disposeObjectTree(loadedRoot);
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
