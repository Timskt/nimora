import { useEffect, useRef } from "react";
import {
  AmbientLight,
  CapsuleGeometry,
  Color,
  DirectionalLight,
  Group,
  MathUtils,
  Mesh,
  MeshPhysicalMaterial,
  OrthographicCamera,
  Scene,
  SphereGeometry,
  Timer,
  TorusGeometry,
  WebGLRenderer,
} from "three";

interface BuiltinPet3DProps {
  state: string;
  emotion: string;
  onFailure(): void;
}

export interface BuiltinPetPose {
  bounce: number;
  bodyTilt: number;
  eyeScale: number;
  tailSpeed: number;
}

export function builtinPetPose(state: string, emotion: string): BuiltinPetPose {
  if (state === "sleeping" || emotion === "sleepy") {
    return { bounce: 0.012, bodyTilt: 0.07, eyeScale: 0.08, tailSpeed: 0.35 };
  }
  if (state === "walking") return { bounce: 0.09, bodyTilt: 0.045, eyeScale: 1, tailSpeed: 2.1 };
  if (state === "celebrate" || state === "playing" || emotion === "happy") {
    return { bounce: 0.14, bodyTilt: 0.075, eyeScale: 1, tailSpeed: 2.7 };
  }
  if (state === "observing" || emotion === "surprised") {
    return { bounce: 0.025, bodyTilt: 0.025, eyeScale: 1.18, tailSpeed: 1.35 };
  }
  return { bounce: 0.025, bodyTilt: 0.018, eyeScale: 1, tailSpeed: 0.85 };
}

export function clampGaze(value: number): number {
  return MathUtils.clamp(value, -1, 1);
}

export function builtinPetBodyYaw(state: string, elapsed: number, gazeX: number): number {
  if (state === "playing" || state === "celebrate") return elapsed * 1.65;
  if (state === "walking") return Math.sin(elapsed * 0.72) >= 0 ? Math.PI * 0.28 : -Math.PI * 0.28;
  if (state === "observing") return Math.sin(elapsed * 0.62) * Math.PI * 0.42;
  return gazeX * 0.16;
}

export function BuiltinPet3D({ state, emotion, onFailure }: BuiltinPet3DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef({ state, emotion });
  stateRef.current = { state, emotion };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let renderer: WebGLRenderer;
    try {
      renderer = new WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "high-performance" });
    } catch {
      onFailure();
      return;
    }
    renderer.setClearColor(0x000000, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

    const scene = new Scene();
    const camera = new OrthographicCamera(-2.15, 2.15, 2.55, -2.15, 0.1, 100);
    camera.position.set(0, 0.12, 8);
    scene.add(new AmbientLight(0xfff7e8, 2.8));
    const keyLight = new DirectionalLight(0xffffff, 4.8);
    keyLight.position.set(-3, 5, 7);
    scene.add(keyLight);
    const rimLight = new DirectionalLight(0x8fdcff, 3.4);
    rimLight.position.set(4, 2, -2);
    scene.add(rimLight);

    const root = new Group();
    root.position.y = -0.12;
    scene.add(root);
    const body = new Group();
    root.add(body);
    const head = new Group();
    head.position.set(0, 0.55, 0.08);
    body.add(head);

    const yellow = material("#f7c928", 0.34, 0.82);
    const yellowLight = material("#ffdf4f", 0.3, 0.86);
    const graphite = material("#20232a", 0.2, 0.78);
    const metal = metallicMaterial("#c5cbd2", 0.18, 0.92);
    const glass = material("#7bdfff", 0.12, 0.92, "#169bc7", 0.28);
    const white = material("#fffdf2", 0.6, 0.34);
    const coral = material("#ff7d69", 0.5, 0.42, "#d83d2c", 0.08);
    const denim = material("#2878cf", 0.62, 0.32);
    const denimLight = material("#4d9ce8", 0.58, 0.38);

    const torso = makeMesh(new CapsuleGeometry(0.86, 0.98, 16, 48), yellow, [0, -0.34, 0], [1.04, 1.08, 0.88]);
    body.add(torso);
    const faceShell = makeMesh(new SphereGeometry(1.02, 56, 40), yellowLight, [0, 0, 0], [1, 0.88, 0.82]);
    head.add(faceShell);

    const visor = makeMesh(new TorusGeometry(0.6, 0.11, 18, 56), graphite, [0, 0.05, 0.77], [1, 1.02, 0.42]);
    head.add(visor);
    const strap = makeMesh(new TorusGeometry(0.91, 0.07, 14, 56), graphite, [0, 0.03, 0], [1, 0.83, 1]);
    strap.rotation.x = Math.PI / 2;
    head.add(strap);
    const eyeGroup = new Group();
    eyeGroup.position.set(0, 0.06, 1.01);
    const eye = makeMesh(new SphereGeometry(0.43, 36, 26), white, [0, 0, 0], [1, 1.06, 0.34]);
    const iris = makeMesh(new SphereGeometry(0.22, 30, 22), glass, [0, -0.005, 0.12], [1, 1, 0.4]);
    const pupil = makeMesh(new SphereGeometry(0.105, 24, 18), graphite, [0, -0.005, 0.2], [1, 1, 0.34]);
    const shine = makeMesh(new SphereGeometry(0.055, 16, 12), white, [-0.045, 0.08, 0.24], [1, 1, 0.25]);
    const lensRing = makeMesh(new TorusGeometry(0.52, 0.095, 18, 56), metal, [0, 0, 0.94], [1, 1.04, 0.45]);
    eyeGroup.add(eye, iris, pupil, shine);
    head.add(lensRing, eyeGroup);

    const mouth = makeMesh(new TorusGeometry(0.19, 0.035, 10, 28, Math.PI), graphite, [0, -0.39, 0.83], [1, 0.72, 0.5]);
    mouth.rotation.z = Math.PI;
    head.add(mouth);
    const cheekLeft = makeMesh(new SphereGeometry(0.085, 18, 12), coral, [-0.62, -0.27, 0.75], [1.4, 0.55, 0.25]);
    const cheekRight = cheekLeft.clone();
    cheekRight.position.x = 0.68;
    head.add(cheekLeft, cheekRight);

    const utilityVest = makeMesh(new SphereGeometry(0.84, 48, 34), denim, [0, -0.68, 0.42], [0.99, 0.76, 0.54]);
    const utilityPocket = makeMesh(new CapsuleGeometry(0.2, 0.22, 10, 28), denimLight, [0, -0.76, 0.89], [1.3, 0.82, 0.22]);
    utilityPocket.rotation.z = Math.PI / 2;
    body.add(utilityVest, utilityPocket);
    for (const side of [-1, 1]) {
      const overallStrap = makeMesh(new CapsuleGeometry(0.075, 0.72, 8, 20), denimLight, [side * 0.48, -0.34, 0.75], [1, 1, 0.42]);
      overallStrap.rotation.z = side * -0.4;
      const button = makeMesh(new SphereGeometry(0.09, 18, 12), metal, [side * 0.34, -0.54, 0.92], [1, 1, 0.35]);
      body.add(overallStrap, button);
    }

    const arms: Group[] = [];
    const feet: Group[] = [];
    for (const side of [-1, 1]) {
      const arm = new Group();
      arm.position.set(side * 0.9, -0.35, 0);
      const upperArm = makeMesh(new CapsuleGeometry(0.14, 0.5, 10, 24), yellow, [side * 0.12, -0.25, 0], [1, 1, 1]);
      upperArm.rotation.z = side * -0.32;
      const mitten = makeMesh(new SphereGeometry(0.23, 24, 18), graphite, [side * 0.24, -0.59, 0.03], [1, 0.86, 0.82]);
      const thumb = makeMesh(new SphereGeometry(0.09, 16, 12), graphite, [side * 0.42, -0.56, 0.08], [1, 0.72, 0.8]);
      arm.add(upperArm, mitten, thumb);
      body.add(arm);
      arms.push(arm);

      const foot = new Group();
      foot.position.set(side * 0.46, -1.37, 0.2);
      const boot = makeMesh(new CapsuleGeometry(0.24, 0.28, 8, 20), graphite, [0, 0, 0], [1.15, 0.72, 1.28]);
      boot.rotation.x = Math.PI / 2;
      foot.add(boot);
      body.add(foot);
      feet.push(foot);
    }

    let gazeX = 0;
    let gazeY = 0;
    const trackPointer = (event: PointerEvent) => {
      gazeX = clampGaze((event.clientX - window.innerWidth / 2) / Math.max(window.innerWidth * 0.42, 1));
      gazeY = clampGaze((event.clientY - window.innerHeight / 2) / Math.max(window.innerHeight * 0.42, 1));
    };
    window.addEventListener("pointermove", trackPointer, { passive: true });

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const timer = new Timer();
    let frame = 0;
    let disposed = false;
    const render = () => {
      if (disposed) return;
      frame = window.requestAnimationFrame(render);
      timer.update();
      const elapsed = timer.getElapsed();
      const current = stateRef.current;
      const pose = builtinPetPose(current.state, current.emotion);
      const motion = reducedMotion.matches ? 0 : 1;
      const walking = current.state === "walking";
      const playing = current.state === "playing" || current.state === "celebrate";
      const sleeping = current.state === "sleeping" || current.emotion === "sleepy";
      root.position.y = -0.12 + Math.abs(Math.sin(elapsed * (walking ? 5.2 : pose.tailSpeed + 0.7))) * pose.bounce * motion;
      body.rotation.z = Math.sin(elapsed * (walking ? 4.4 : 1.4)) * pose.bodyTilt * motion;
      const targetYaw = builtinPetBodyYaw(current.state, elapsed, gazeX) * motion;
      body.rotation.y = playing ? targetYaw : MathUtils.lerp(body.rotation.y, targetYaw, 0.055);
      head.rotation.y = MathUtils.lerp(head.rotation.y, gazeX * 0.28 - body.rotation.y * 0.18, 0.08);
      head.rotation.x = MathUtils.lerp(head.rotation.x, gazeY * 0.12 + (sleeping ? 0.12 : 0), 0.08);
      const blink = sleeping ? 0.08 : Math.sin(elapsed * 1.08) > 0.986 ? 0.08 : pose.eyeScale;
      eyeGroup.scale.y = MathUtils.lerp(eyeGroup.scale.y, blink, 0.38);
      for (let index = 0; index < arms.length; index += 1) {
        const direction = index === 0 ? 1 : -1;
        const target = playing ? direction * (0.65 + Math.sin(elapsed * 5 + index) * 0.2) : walking ? direction * Math.sin(elapsed * 7) * 0.34 : direction * 0.05;
        arms[index]!.rotation.z = MathUtils.lerp(arms[index]!.rotation.z, target * motion, 0.14);
        feet[index]!.position.y = -1.37 + (walking ? Math.sin(elapsed * 8 + index * Math.PI) * 0.1 * motion : 0);
        feet[index]!.rotation.z = walking ? Math.sin(elapsed * 8 + index * Math.PI) * 0.14 * motion : 0;
      }
      renderer.render(scene, camera);
    };

    const resize = () => renderer.setSize(Math.max(canvas.clientWidth, 1), Math.max(canvas.clientHeight, 1), false);
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();
    frame = window.requestAnimationFrame(render);
    return () => {
      disposed = true;
      window.cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", trackPointer);
      observer.disconnect();
      scene.traverse((object) => {
        if (!(object instanceof Mesh)) return;
        object.geometry.dispose();
        const materials = Array.isArray(object.material) ? object.material : [object.material];
        for (const value of materials) value.dispose();
      });
      renderer.dispose();
    };
  }, [onFailure]);

  return <canvas ref={canvasRef} className="builtin-pet-3d" aria-label="Aster 3D Q 版黄色桌面伙伴" />;
}

function material(color: string, roughness: number, clearcoat: number, emissive = "#000000", emissiveIntensity = 0): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color: new Color(color), roughness, clearcoat, clearcoatRoughness: 0.34, emissive: new Color(emissive), emissiveIntensity });
}

function metallicMaterial(color: string, roughness: number, clearcoat: number): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color: new Color(color), roughness, metalness: 0.78, clearcoat, clearcoatRoughness: 0.16 });
}

function makeMesh(
  geometry: SphereGeometry | CapsuleGeometry | TorusGeometry,
  meshMaterial: MeshPhysicalMaterial,
  position: [number, number, number],
  scale: [number, number, number],
): Mesh {
  const value = new Mesh(geometry, meshMaterial);
  value.position.set(...position);
  value.scale.set(...scale);
  return value;
}
