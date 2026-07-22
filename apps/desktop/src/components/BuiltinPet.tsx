import { useEffect, useRef, useState } from "react";
import type { GazeOffset } from "./petGaze";
import { NEUTRAL_GAZE } from "./petGaze";
import { createFollower, stepFollower, type FollowerConfig } from "./petSecondaryMotion";
import { nextBlink } from "./petBlink";

interface BuiltinPetProps {
  state: string;
  emotion: string;
  /**
   * Pupil displacement, in this SVG's own coordinate units, that makes the eyes
   * track the pointer. Defaults to a neutral forward gaze.
   */
  gaze?: GazeOffset;
  /**
   * When true, secondary motion (the lagging tail swing) and blinking are
   * suppressed and the eyes/tail rest, honoring the reduced-motion preference.
   */
  reducedMotion?: boolean;
  /**
   * Pet energy, 0..100. Low energy makes the pet blink rarely and heavily (the
   * drowsy "dry-eye" tell); defaults to fully rested.
   */
  energy?: number;
}

/**
 * Soft, trailing tail: low stiffness so it lags the body, high retained
 * velocity so it swings past and settles rather than snapping.
 */
const TAIL_FOLLOWER: FollowerConfig = { stiffness: 0.18, damping: 0.82, maxLeash: 12 };
/** Maps a horizontal gaze lean (SVG units) to the tail's anchor target. */
const TAIL_LEAN_GAIN = 1.6;
/** Degrees of tail rotation per unit of follower displacement. */
const TAIL_DEGREES_PER_UNIT = 1.1;

export function BuiltinPet({
  state,
  emotion,
  gaze = NEUTRAL_GAZE,
  reducedMotion = false,
  energy = 100,
}: BuiltinPetProps) {
  const gazeTransform = `translate(${gaze.dx} ${gaze.dy})`;

  // Secondary motion: the tail trails the pet's look direction. The anchor
  // target is derived from the horizontal gaze lean and lives in a ref so the
  // rAF loop always reads the latest value without re-subscribing each frame.
  const [tailAngle, setTailAngle] = useState(0);
  const anchorX = useRef(0);
  anchorX.current = reducedMotion ? 0 : gaze.dx * TAIL_LEAN_GAIN;

  useEffect(() => {
    if (reducedMotion || typeof window.requestAnimationFrame !== "function") {
      setTailAngle(0);
      return;
    }
    let follower = createFollower({ x: 0, y: 0 });
    let frame = 0;
    const tick = () => {
      follower = stepFollower(follower, { x: anchorX.current, y: 0 }, TAIL_FOLLOWER);
      setTailAngle(follower.position.x * TAIL_DEGREES_PER_UNIT);
      frame = window.requestAnimationFrame(tick);
    };
    frame = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(frame);
  }, [reducedMotion]);

  const tailTransform = `rotate(${tailAngle.toFixed(2)} 168 200)`;
  return (
    <svg
      className={`builtin-pet overlay-pet ${state} emotion-${emotion}`}
      viewBox="0 0 220 240"
      role="img"
      aria-label="Aster 内置桌面伙伴"
    >
      <defs>
        <linearGradient id="aster-fur" x1="38" y1="26" x2="178" y2="218" gradientUnits="userSpaceOnUse">
          <stop stopColor="#fffdf8" />
          <stop offset="0.52" stopColor="#eee8fb" />
          <stop offset="1" stopColor="#c9bce9" />
        </linearGradient>
        <linearGradient id="aster-ear" x1="0" y1="0" x2="1" y2="1">
          <stop stopColor="#f8d4df" />
          <stop offset="1" stopColor="#d7c4ed" />
        </linearGradient>
        <radialGradient id="aster-eye" cx="38%" cy="28%" r="70%">
          <stop stopColor="#7568b9" />
          <stop offset="0.55" stopColor="#41376f" />
          <stop offset="1" stopColor="#201b3d" />
        </radialGradient>
        <filter id="aster-shadow" x="-40%" y="-40%" width="180%" height="200%">
          <feDropShadow dx="0" dy="10" stdDeviation="8" floodColor="#392d69" floodOpacity=".22" />
        </filter>
      </defs>
      <g className="builtin-tail">
        <g className="builtin-tail-swing" transform={tailTransform}>
          <path d="M168 157c36 3 43 38 22 54-12 9-28 5-27-8 1-8 10-9 16-14 7-7 3-17-8-19" fill="none" stroke="#c9bce9" strokeWidth="20" strokeLinecap="round" />
          <path d="M177 169c10 4 14 13 10 21" fill="none" stroke="#e9e0f7" strokeWidth="7" strokeLinecap="round" opacity=".72" />
        </g>
      </g>
      <g className="builtin-body" filter="url(#aster-shadow)">
        <path className="builtin-ear builtin-ear-left" d="M50 73 43 18c-1-8 7-12 13-7l40 38Z" fill="url(#aster-fur)" stroke="#b9abd9" strokeWidth="2" />
        <path className="builtin-ear builtin-ear-right" d="m126 48 39-37c6-5 14-1 13 7l-7 55Z" fill="url(#aster-fur)" stroke="#b9abd9" strokeWidth="2" />
        <path d="m54 57-5-31 29 28Z" fill="url(#aster-ear)" opacity=".88" />
        <path d="m148 54 27-28-5 31Z" fill="url(#aster-ear)" opacity=".88" />
        <path d="M44 111c0-43 27-70 66-70s66 27 66 70v58c0 37-25 58-66 58s-66-21-66-58Z" fill="url(#aster-fur)" stroke="#b9abd9" strokeWidth="2" />
        <path d="M55 106c4-29 22-51 48-58-31 2-52 26-52 61v59c0 29 15 47 43 54-24-15-39-53-39-116Z" fill="#fff" opacity=".3" />
        <g className="builtin-markings" fill="none" stroke="#9a82c7" strokeLinecap="round" opacity=".27">
          <path d="m96 57 8 13" strokeWidth="6" /><path d="m112 54-1 17" strokeWidth="6" /><path d="m128 58-9 13" strokeWidth="6" />
        </g>
        <g className="builtin-face">
          <ellipse className="builtin-eye builtin-eye-left" cx="82" cy="112" rx="11" ry="16" fill="url(#aster-eye)" />
          <ellipse className="builtin-eye builtin-eye-right" cx="138" cy="112" rx="11" ry="16" fill="url(#aster-eye)" />
          <g className="builtin-pupils" transform={gazeTransform}>
            <circle className="builtin-pupil" cx="82" cy="112" r="4.4" fill="#201b3d" />
            <circle className="builtin-pupil" cx="138" cy="112" r="4.4" fill="#201b3d" />
            <circle cx="78" cy="106" r="3.6" fill="#fff" /><circle cx="134" cy="106" r="3.6" fill="#fff" />
          </g>
          <ellipse className="builtin-blush" cx="66" cy="139" rx="15" ry="7" fill="#ee9ab6" opacity=".2" />
          <ellipse className="builtin-blush" cx="154" cy="139" rx="15" ry="7" fill="#ee9ab6" opacity=".2" />
          <path d="m104 132 6 5 6-5c0-5-12-5-12 0Z" fill="#77658c" />
          <path className="builtin-mouth" d="M110 137c-1 8-12 9-14 3m14-3c1 8 12 9 14 3" fill="none" stroke="#655778" strokeWidth="2.6" strokeLinecap="round" />
          <path d="M54 128H31m25 10-22 6m132-16h23m-25 10 22 6" fill="none" stroke="#807391" strokeWidth="2" strokeLinecap="round" opacity=".55" />
        </g>
        <g className="builtin-chest">
          <path d="M82 161c13 7 43 7 56 0-1 25-10 43-28 43s-27-18-28-43Z" fill="#fff" opacity=".45" />
          <path d="m101 169 9 8 9-8" fill="none" stroke="#aa97ce" strokeWidth="3" strokeLinecap="round" />
        </g>
        <g className="builtin-paws">
          <ellipse cx="76" cy="208" rx="27" ry="17" fill="#e7def7" stroke="#b9abd9" strokeWidth="2" />
          <ellipse cx="144" cy="208" rx="27" ry="17" fill="#e7def7" stroke="#b9abd9" strokeWidth="2" />
          <path d="M66 208h20m48 0h20" stroke="#b7a4d7" strokeWidth="2" strokeLinecap="round" opacity=".55" />
        </g>
        <g className="builtin-star">
          <path d="m110 79 4 9 10 1-8 7 2 10-8-5-8 5 2-10-8-7 10-1Z" fill="#f2bd63" stroke="#fff4cd" strokeWidth="2" />
        </g>
      </g>
    </svg>
  );
}
