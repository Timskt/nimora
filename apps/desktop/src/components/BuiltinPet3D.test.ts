import { describe, expect, it } from "vitest";
import {
  builtinPetBodyYaw,
  builtinPetPose,
  clampGaze,
  createSpringState,
  cycleEnvelope,
  idleMicroActIntensity,
  idlePerformancePhase,
  isClassicQMinionSilhouette,
  isDazedState,
  isSadEmotion,
  normalizeMotionState,
  Q_MINION_COLORS,
  Q_MINION_LAYOUT,
  qMinionGogglePose,
  qMinionHairTuftSpecs,
  qMinionOverallStrapPose,
  sampleBuiltinPetMotion,
  springToward,
  stepSpringDamper,
} from "./BuiltinPet3D";

describe("BuiltinPet3D behavior", () => {
  it("keeps pointer gaze within a safe head rotation range", () => {
    expect(clampGaze(-4)).toBe(-1);
    expect(clampGaze(0.35)).toBe(0.35);
    expect(clampGaze(9)).toBe(1);
  });

  it("uses the third dimension for turning and full play spins", () => {
    expect(Math.abs(builtinPetBodyYaw("walking", 1, 0))).toBeGreaterThan(0.8);
    expect(Math.abs(builtinPetBodyYaw("observing", 2, 0))).toBeGreaterThan(0.5);
    expect(builtinPetBodyYaw("playing", Math.PI * 2 / 1.35, 0)).toBeCloseTo(Math.PI * 2);
  });

  it("gives active and resting states distinct motion", () => {
    expect(builtinPetPose("walking", "happy").bounce).toBeGreaterThan(builtinPetPose("idle", "neutral").bounce);
    expect(builtinPetPose("playing", "happy").bounce).toBeGreaterThan(builtinPetPose("walking", "neutral").bounce);
    expect(builtinPetPose("sleeping", "sleepy").eyeScale).toBeLessThan(0.1);
    expect(builtinPetPose("observing", "surprised").eyeScale).toBeGreaterThan(1);
    expect(builtinPetPose("work", "neutral").armRest).toBeGreaterThan(builtinPetPose("idle", "neutral").armRest);
    expect(builtinPetPose("celebrate", "happy").hop).toBeGreaterThan(0);
    expect(builtinPetPose("work", "neutral").slump).toBeGreaterThan(builtinPetPose("idle", "neutral").slump);
    expect(builtinPetPose("work", "neutral").sweat).toBeGreaterThan(0.5);
  });

  it("detects crash / wounded daze states", () => {
    expect(isDazedState("work_crash", "neutral")).toBe(true);
    expect(isDazedState("idle", "wounded")).toBe(true);
    expect(isDazedState("idle", "neutral")).toBe(false);
    expect(builtinPetPose("crash", "neutral").eyeScale).toBeLessThan(0.9);
  });

  it("marks connector-offline mood as sad (not crash daze)", () => {
    expect(isSadEmotion("sad")).toBe(true);
    expect(isSadEmotion("lonely")).toBe(true);
    expect(isSadEmotion("neutral")).toBe(false);
    expect(isDazedState("idle", "sad")).toBe(false);
    expect(builtinPetPose("idle", "sad").slump).toBeGreaterThan(builtinPetPose("idle", "neutral").slump);
    const sad = sampleBuiltinPetMotion("idle", "sad", 1.2, 0, 0, 1);
    const neutral = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0, 0, 1);
    expect(sad.mouthSmile).toBeLessThan(0);
    expect(sad.mouthSmile).toBeLessThan(neutral.mouthSmile);
    expect(sad.headPitch).toBeGreaterThan(neutral.headPitch);
  });

  it("exposes idle micro-performance gates", () => {
    const phase = idlePerformancePhase(0);
    expect(phase.weightShift).toBeGreaterThanOrEqual(0);
    expect(phase.weightShift).toBeLessThanOrEqual(1);
    expect(phase.yawn).toBeGreaterThanOrEqual(0);
    expect(phase.digNose).toBeGreaterThanOrEqual(0);
    expect(phase.countAnts).toBeGreaterThanOrEqual(0);
    expect(phase.countAnts).toBeLessThanOrEqual(1);
    expect(typeof phase.blink).toBe("boolean");
    expect(typeof phase.blinkL).toBe("boolean");
    expect(typeof phase.blinkR).toBe("boolean");
    expect(phase.cheekPulse).toBeGreaterThanOrEqual(0);
    expect(phase.cheekPulse).toBeLessThanOrEqual(1);
    expect(phase.fidget).toBeGreaterThanOrEqual(0);
    expect(phase.settleHop).toBeGreaterThanOrEqual(0);
    expect(phase.waveLite).toBeGreaterThanOrEqual(0);
    expect(phase.hopLite).toBeGreaterThanOrEqual(0);

    let foundYawn = false;
    let foundAnts = false;
    let foundFidget = false;
    let foundWave = false;
    for (let t = 0; t < 300; t += 0.25) {
      const p = idlePerformancePhase(t);
      if (p.yawn > 0.4) foundYawn = true;
      if (p.countAnts > 0.4) foundAnts = true;
      if (p.fidget > 0.4) foundFidget = true;
      if (p.waveLite > 0.4) foundWave = true;
    }
    expect(foundYawn).toBe(true);
    expect(foundAnts).toBe(true);
    expect(foundFidget).toBe(true);
    expect(foundWave).toBe(true);
  });

  it("keeps a readable micro-act at least every 3s while idle", () => {
    // cycleEnvelope peaks deterministically; composite intensity must not go dark for >3s.
    expect(cycleEnvelope(2.15, 2.4, 1.98, 2.34)).toBeGreaterThan(0.4);
    let lastPeak = 0;
    let maxGap = 0;
    let sawPeak = false;
    for (let t = 0; t < 180; t += 0.05) {
      const intensity = idleMicroActIntensity(t);
      if (intensity > 0.35) {
        if (sawPeak) maxGap = Math.max(maxGap, t - lastPeak);
        lastPeak = t;
        sawPeak = true;
      }
    }
    expect(sawPeak).toBe(true);
    expect(maxGap).toBeLessThanOrEqual(3);
  });


  it("keeps idle look-around and micro-bounce alive (never a static pose)", () => {
    let maxAbsSweep = 0;
    let maxBounce = 0;
    let minBounce = 1;
    let foundDig = false;
    let foundSettle = false;
    for (let t = 0; t < 200; t += 0.2) {
      const phase = idlePerformancePhase(t);
      maxAbsSweep = Math.max(maxAbsSweep, Math.abs(phase.lookSweep));
      maxBounce = Math.max(maxBounce, phase.microBounce);
      minBounce = Math.min(minBounce, phase.microBounce);
      if (phase.digNose > 0.45) foundDig = true;
      if (phase.settleHop > 0.45) foundSettle = true;
    }
    expect(maxAbsSweep).toBeGreaterThan(0.5);
    // Continuous micro-bounce: not stuck at 0 or 1 across the window.
    expect(maxBounce).toBeGreaterThan(0.7);
    expect(minBounce).toBeLessThan(0.55);
    expect(foundDig).toBe(true);
    expect(foundSettle).toBe(true);

    // lookSweep couples into idle head yaw / iris so look-around is visible.
    let maxHeadYaw = 0;
    let maxIrisX = 0;
    for (let t = 0; t < 80; t += 0.25) {
      const sample = sampleBuiltinPetMotion("idle", "neutral", t, 0, 0, 1);
      maxHeadYaw = Math.max(maxHeadYaw, Math.abs(sample.headYaw));
      maxIrisX = Math.max(maxIrisX, Math.abs(sample.irisX));
    }
    expect(maxHeadYaw).toBeGreaterThan(0.1);
    expect(maxIrisX).toBeGreaterThan(0.012);

    // Pointer gaze pulls head/iris more strongly than neutral idle.
    const look = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0.85, -0.4, 1);
    const rest = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0, 0, 1);
    expect(look.headYaw).toBeGreaterThan(rest.headYaw);
    expect(look.irisX).toBeGreaterThan(rest.irisX);
    expect(look.headPitch).toBeLessThan(rest.headPitch);
  });

  it("drives count-ants gaze-down lean and arm point only while idle", () => {
    let peakAnts = 0;
    let peakT = 0;
    for (let t = 0; t < 300; t += 0.2) {
      const ants = idlePerformancePhase(t).countAnts;
      if (ants > peakAnts) {
        peakAnts = ants;
        peakT = t;
      }
    }
    expect(peakAnts).toBeGreaterThan(0.45);

    const idle = sampleBuiltinPetMotion("idle", "neutral", peakT, 0, 0, 1);
    const baseline = sampleBuiltinPetMotion("idle", "neutral", 0.5, 0, 0, 1);
    const work = sampleBuiltinPetMotion("work", "neutral", peakT, 0, 0, 1);
    const reduced = sampleBuiltinPetMotion("idle", "neutral", peakT, 0, 0, 0);

    expect(idle.countAnts).toBeGreaterThan(0.35);
    expect(idle.headPitch).toBeGreaterThan(baseline.headPitch + 0.04);
    expect(idle.irisY).toBeGreaterThan(baseline.irisY);
    expect(Math.abs(idle.bodyTilt)).toBeGreaterThan(Math.abs(baseline.bodyTilt) * 0.5);
    // Occasional point: right arm diverges while ants is hot.
    expect(Math.abs(idle.armR - baseline.armR) + Math.abs(idle.armL - baseline.armL)).toBeGreaterThan(0.05);
    // Non-idle states suppress the ants performance channel.
    expect(work.countAnts).toBe(0);
    // Reduced motion stays near identity (no ants lean / pitch).
    expect(reduced.countAnts).toBeCloseTo(0);
    expect(reduced.headPitch).toBeCloseTo(0);
    expect(reduced.bodyTilt).toBeCloseTo(0);
    expect(reduced.scaleX).toBeCloseTo(1);
    expect(reduced.scaleY).toBeCloseTo(1);
  });

  it("samples work sweat scale for working posture", () => {
    const work = sampleBuiltinPetMotion("work", "neutral", 2.4, 0, 0, 1);
    const idle = sampleBuiltinPetMotion("idle", "neutral", 2.4, 0, 0, 1);
    const reduced = sampleBuiltinPetMotion("work", "neutral", 2.4, 0, 0, 0);

    expect(work.sweat).toBeGreaterThan(0.3);
    expect(work.sweat).toBeGreaterThan(idle.sweat);
    expect(work.headPitch).toBeGreaterThan(idle.headPitch);
    expect(reduced.sweat).toBeCloseTo(0);
  });

  it("opens mouth on idle yawn micro-performance", () => {
    let maxMouth = 0;
    let maxYawn = 0;
    for (let t = 0; t < 200; t += 0.2) {
      const phase = idlePerformancePhase(t);
      maxYawn = Math.max(maxYawn, phase.yawn);
      if (phase.yawn > 0.5) {
        const sample = sampleBuiltinPetMotion("idle", "neutral", t, 0, 0, 1);
        maxMouth = Math.max(maxMouth, sample.mouthOpen);
      }
    }
    expect(maxYawn).toBeGreaterThan(0.5);
    expect(maxMouth).toBeGreaterThan(0.25);
  });

  it("samples celebrate hop, arm raise, and sparkle", () => {
    const celebratePose = builtinPetPose("celebrate", "happy");
    const playPose = builtinPetPose("playing", "happy");
    const idlePose = builtinPetPose("idle", "neutral");
    // Hop lives on BuiltinPetPose — never on BuiltinPetMotionSample.
    expect(celebratePose.hop).toBeGreaterThan(0);
    expect(playPose.hop).toBeGreaterThan(0);
    expect(idlePose.hop).toBe(0);
    expect(celebratePose.stretch).toBeGreaterThan(idlePose.stretch);
    expect(celebratePose.squash).toBeLessThan(idlePose.squash);

    const celebrate = sampleBuiltinPetMotion("celebrate", "happy", 1.2, 0, 0, 1);
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0, 0, 1);
    const play = sampleBuiltinPetMotion("playing", "happy", 0.8, 0, 0, 1);

    // Peak hop over a short window: pose.hop drives rootY lift + squash/stretch.
    let maxRootY = -Infinity;
    let maxStretchSpread = 0;
    for (let t = 0; t < 2; t += 0.05) {
      const s = sampleBuiltinPetMotion("celebrate", "happy", t, 0, 0, 1);
      maxRootY = Math.max(maxRootY, s.rootY);
      maxStretchSpread = Math.max(maxStretchSpread, Math.abs(s.scaleY - s.scaleX));
    }
    expect(maxRootY).toBeGreaterThan(idle.rootY);
    expect(maxStretchSpread).toBeGreaterThan(0.02);
    expect(Math.abs(celebrate.armL)).toBeGreaterThan(Math.abs(idle.armL));
    expect(celebrate.sparkle).toBeGreaterThan(0.3);
    expect(play.sparkle).toBeGreaterThan(0);
    // Hop wave lifts root relative to non-hopping sleep crouch.
    const sleep = sampleBuiltinPetMotion("sleeping", "sleepy", 1.2, 0, 0, 1);
    expect(celebrate.rootY).toBeGreaterThan(sleep.rootY);
  });

  it("drives dual-eye shared gaze and blink channels", () => {
    // Dual goggles share iris X/Y; vertical scale can diverge for asymmetric blinks.
    const lookLeft = sampleBuiltinPetMotion("observing", "surprised", 1.2, -0.85, 0.35, 1);
    const lookRight = sampleBuiltinPetMotion("observing", "surprised", 1.2, 0.85, -0.35, 1);
    const sleep = sampleBuiltinPetMotion("sleeping", "sleepy", 1.2, 0, 0, 1);
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0.2, -0.1, 1);
    const idleHardGaze = sampleBuiltinPetMotion("idle", "neutral", 1.2, -0.85, 0.35, 1);
    const reduced = sampleBuiltinPetMotion("observing", "surprised", 1.2, 0.85, 0.35, 0);

    expect(lookLeft.irisX).toBeLessThan(0);
    expect(lookRight.irisX).toBeGreaterThan(0);
    expect(lookLeft.irisY).toBeGreaterThan(lookRight.irisY);
    // Same pointer: observing amplifies iris travel over idle.
    expect(Math.abs(lookLeft.irisX)).toBeGreaterThan(Math.abs(idleHardGaze.irisX));
    // Mild idle gaze still tracks (alive, not frozen).
    expect(idle.irisX).toBeGreaterThan(0);
    // Sleep collapses shared vertical eye scale used by both goggle groups.
    expect(sleep.eyeScaleY).toBeLessThan(0.15);
    expect(sleep.eyeScaleYL).toBeLessThan(0.15);
    expect(sleep.eyeScaleYR).toBeLessThan(0.15);
    expect(idle.eyeScaleY).toBeGreaterThan(0.5);
    // eyeScaleY is the mean of the dual goggle scales.
    expect(idle.eyeScaleY).toBeCloseTo((idle.eyeScaleYL + idle.eyeScaleYR) * 0.5, 5);
    // Reduced motion zeros gaze exports for both eyes.
    expect(reduced.irisX).toBeCloseTo(0);
    expect(reduced.irisY).toBeCloseTo(0);
    expect(reduced.headYaw).toBeCloseTo(0);
    expect(reduced.eyeScaleYL).toBeCloseTo(reduced.eyeScaleYR);
  });

  it("exports hair sway from rootY (not pose bounce) and cheek flush coupling", () => {
    // sample.bounce must never exist — hop lives on BuiltinPetPose only.
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.4, 0.3, -0.15, 1);
    const work = sampleBuiltinPetMotion("work", "neutral", 1.4, 0, 0, 1);
    const celebrate = sampleBuiltinPetMotion("celebrate", "happy", 0.6, 0, 0, 1);
    const reduced = sampleBuiltinPetMotion("idle", "neutral", 1.4, 0.3, -0.15, 0);

    expect("bounce" in idle).toBe(false);
    expect(typeof idle.hairSway).toBe("number");
    expect(typeof idle.cheekFlush).toBe("number");
    // Hair sway couples to head pitch / root hop targets.
    expect(Math.abs(idle.hairSway)).toBeGreaterThan(0);
    // Work stress + celebrate raise cheek flush above quiet idle baseline.
    expect(work.cheekFlush).toBeGreaterThan(0.1);
    expect(celebrate.cheekFlush).toBeGreaterThan(idle.cheekFlush);
    expect(work.sweat).toBeGreaterThan(0.3);
    // Reduced motion zeros secondary expressive channels.
    expect(reduced.hairSway).toBeCloseTo(0);
    expect(reduced.cheekFlush).toBeCloseTo(0);
    expect(reduced.sweat).toBeCloseTo(0);
  });

  it("produces asymmetric blink windows across idle time", () => {
    let asymmetric = false;
    let anyBlink = false;
    for (let t = 0; t < 120; t += 0.05) {
      const phase = idlePerformancePhase(t);
      if (phase.blinkL || phase.blinkR) anyBlink = true;
      if (phase.blinkL !== phase.blinkR) asymmetric = true;
      const sample = sampleBuiltinPetMotion("idle", "neutral", t, 0, 0, 1);
      if (Math.abs(sample.eyeScaleYL - sample.eyeScaleYR) > 0.02) asymmetric = true;
    }
    expect(anyBlink).toBe(true);
    expect(asymmetric).toBe(true);
  });

  it("samples squash/stretch and secondary limb motion", () => {
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.5, 0.2, -0.1, 1);
    const play = sampleBuiltinPetMotion("playing", "happy", 1.5, 0, 0, 1);
    const sleep = sampleBuiltinPetMotion("sleeping", "sleepy", 1.5, 0, 0, 1);
    const reduced = sampleBuiltinPetMotion("playing", "happy", 1.5, 0, 0, 0);
    const observe = sampleBuiltinPetMotion("observing", "surprised", 2.0, 0.6, -0.3, 1);

    expect(play.scaleY).not.toBe(1);
    expect(Math.abs(play.armL)).toBeGreaterThan(Math.abs(idle.armL) * 0.5);
    expect(sleep.eyeScaleY).toBeLessThan(0.2);
    expect(sleep.shadowOpacity).toBeGreaterThan(0.3);
    expect(reduced.scaleX).toBeCloseTo(1);
    expect(reduced.bodyTilt).toBeCloseTo(0);
    expect(idle.irisX).not.toBe(0);
    expect(Math.abs(observe.irisX)).toBeGreaterThan(Math.abs(idle.irisX));
    expect(Math.abs(observe.headYaw)).toBeGreaterThan(0.1);
  });

  it("grounds shadow denser when crouching / working", () => {
    const work = sampleBuiltinPetMotion("work", "neutral", 1, 0, 0, 1);
    const celebrate = sampleBuiltinPetMotion("celebrate", "happy", 0.3, 0, 0, 1);
    // At hop peak celebrate should shrink shadow relative to work stance.
    let minCelebrateShadow = 2;
    for (let t = 0; t < 2; t += 0.05) {
      minCelebrateShadow = Math.min(minCelebrateShadow, sampleBuiltinPetMotion("celebrate", "happy", t, 0, 0, 1).shadowScale);
    }
    expect(work.shadowScale).toBeGreaterThan(minCelebrateShadow);
    expect(celebrate.shadowOpacity).toBeGreaterThan(0.35);
  });
});

describe("spring-damper secondary motion", () => {
  it("moves toward the target without linear lerp stepping", () => {
    const state = createSpringState(0);
    const a = stepSpringDamper(state, 1, 1 / 60, 14, 0.82);
    const b = stepSpringDamper(a, 1, 1 / 60, 14, 0.82);
    expect(a.value).toBeGreaterThan(0);
    expect(a.value).toBeLessThan(1);
    expect(b.value).toBeGreaterThan(a.value);
    // Velocity is carried between steps (spring, not lerp).
    expect(Math.abs(a.velocity)).toBeGreaterThan(0);
    expect(b.velocity).not.toBe(a.velocity);
  });

  it("settles near the target after many steps with critical-ish damping", () => {
    const state = createSpringState(0);
    for (let i = 0; i < 180; i += 1) {
      const next = stepSpringDamper(state, 1, 1 / 60, 16, 0.95);
      state.value = next.value;
      state.velocity = next.velocity;
    }
    expect(state.value).toBeGreaterThan(0.98);
    expect(Math.abs(state.velocity)).toBeLessThan(0.05);
  });

  it("mutates spring state via springToward helper", () => {
    const state = createSpringState(-0.5);
    const value = springToward(state, 0.5, 1 / 30, 12, 0.85);
    expect(value).toBe(state.value);
    expect(state.value).toBeGreaterThan(-0.5);
  });
});


describe("directive motion vocabulary", () => {
  it("treats work_busy and pet.work as working motion", () => {
    const busy = sampleBuiltinPetMotion("work_busy", "focused", 1.0, 0, 0, 1);
    const work = sampleBuiltinPetMotion("pet.work", "focused", 1.0, 0, 0, 1);
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.0, 0, 0, 1);
    expect(busy.sweat).toBeGreaterThan(idle.sweat);
    expect(work.sweat).toBeGreaterThan(idle.sweat);
  });

  it("treats work_crash as dazed", () => {
    expect(isDazedState("work_crash", "neutral")).toBe(true);
    expect(isDazedState("pet.work_crash", "neutral")).toBe(true);
    const crash = sampleBuiltinPetMotion("work_crash", "wounded", 0.5, 0, 0, 1);
    expect(crash.dazed).toBeGreaterThan(0.5);
  });
});

describe("Q-minion silhouette palette", () => {
  it("uses warm yellow, soft denim, brown boots, and dual goggles layout", () => {
    expect(Q_MINION_COLORS.yellow.toUpperCase()).toBe("#F7D117");
    expect(Q_MINION_COLORS.denim).toMatch(/^#[0-9A-Fa-f]{6}$/);
    expect(Q_MINION_COLORS.boot).toMatch(/^#[0-9A-Fa-f]{6}$/);
    expect(Q_MINION_COLORS.boot.toLowerCase()).not.toBe("#000000");
    expect(Q_MINION_COLORS.glove.toLowerCase()).not.toBe(Q_MINION_COLORS.boot.toLowerCase());
    // Rounder Q pill: body length not much taller than diameter.
    expect(Q_MINION_LAYOUT.bodyLength).toBeLessThan(Q_MINION_LAYOUT.bodyRadius * 1.35);
    expect(Q_MINION_LAYOUT.goggleRadius).toBeGreaterThan(0.32);
    expect(Q_MINION_LAYOUT.goggleSpacing).toBeGreaterThan(0.3);
    expect(Q_MINION_LAYOUT.goggleSpacing).toBeLessThan(0.5);
    expect(Q_MINION_LAYOUT.bootY).toBeLessThan(-1.2);
    expect(Q_MINION_LAYOUT.shadowY).toBeLessThan(Q_MINION_LAYOUT.bootY);
  });

  it("keeps hop on pose and hair sway free of bounce field", () => {
    const pose = builtinPetPose("celebrate", "happy");
    const sample = sampleBuiltinPetMotion("celebrate", "happy", 0.4, 0, 0, 1);
    expect(pose.hop).toBeGreaterThan(0);
    expect("bounce" in sample).toBe(false);
    expect("hop" in sample).toBe(false);
    expect(typeof sample.hairSway).toBe("number");
    expect(Math.abs(sample.hairSway)).toBeGreaterThanOrEqual(0);
  });
});


describe("Q-minion classic silhouette helpers", () => {
  it("recognizes classic dual-goggle pill silhouette", () => {
    expect(isClassicQMinionSilhouette()).toBe(true);
    expect(isClassicQMinionSilhouette({
      ...Q_MINION_LAYOUT,
      bodyLength: 0.2,
      bodyRadius: 1.0,
      goggleRadius: 0.1,
      goggleSpacing: 0.05,
    })).toBe(false);
  });

  it("places dual goggles and overall straps symmetrically", () => {
    const left = qMinionGogglePose(-1);
    const right = qMinionGogglePose(1);
    expect(left[0]).toBeCloseTo(-right[0], 5);
    expect(left[2]).toBe(right[2]);
    const strapL = qMinionOverallStrapPose(-1);
    const strapR = qMinionOverallStrapPose(1);
    expect(strapL.strap[0]).toBeCloseTo(-strapR.strap[0], 5);
    expect(strapL.rotationZ).toBeCloseTo(-strapR.rotationZ, 5);
    expect(qMinionHairTuftSpecs().length).toBeGreaterThanOrEqual(4);
  });
});

describe("directive micro-performance tokens", () => {
  it("normalizes pet.* micro-performance tokens", () => {
    expect(normalizeMotionState("pet.yawn")).toBe("yawn");
    expect(normalizeMotionState("yawn")).toBe("yawn");
    expect(normalizeMotionState("pet.dig_nose")).toBe("dig_nose");
    expect(normalizeMotionState("dig-nose")).toBe("dig_nose");
    expect(normalizeMotionState("pet.count_ants")).toBe("count_ants");
    expect(normalizeMotionState("pet.wave")).toBe("wave");
    expect(normalizeMotionState("pet.look_around")).toBe("look_around");
    expect(normalizeMotionState("look-around")).toBe("look_around");
    expect(normalizeMotionState("pet.hop")).toBe("hop");
  });

  it("amplifies yawn mouth channel for pet.yawn", () => {
    const directed = sampleBuiltinPetMotion("pet.yawn", "neutral", 1.2, 0, 0, 1);
    const idle = sampleBuiltinPetMotion("idle", "neutral", 1.2, 0, 0, 1);
    expect(directed.mouthOpen).toBeGreaterThan(0.45);
    expect(directed.mouthOpen).toBeGreaterThan(idle.mouthOpen);
    expect(builtinPetPose("pet.yawn", "neutral").eyeScale).toBeLessThan(
      builtinPetPose("idle", "neutral").eyeScale,
    );
  });

  it("drives dig-nose pose from pet.dig_nose", () => {
    const sample = sampleBuiltinPetMotion("pet.dig_nose", "neutral", 2, 0, 0, 1);
    expect(sample.digNose).toBeGreaterThan(0.7);
    expect(Math.abs(sample.headPitch)).toBeGreaterThan(0.05);
  });

  it("leans for pet.count_ants", () => {
    const sample = sampleBuiltinPetMotion("pet.count_ants", "neutral", 3, 0, 0, 1);
    expect(sample.countAnts).toBeGreaterThan(0.7);
    expect(sample.headPitch).toBeGreaterThan(
      sampleBuiltinPetMotion("idle", "neutral", 3, 0, 0, 1).headPitch,
    );
  });

  it("raises an arm for pet.wave celebrate-lite", () => {
    let maxArm = 0;
    for (let t = 0; t < 4; t += 0.1) {
      const sample = sampleBuiltinPetMotion("pet.wave", "happy", t, 0, 0, 1);
      maxArm = Math.max(maxArm, sample.armL);
    }
    expect(maxArm).toBeGreaterThan(0.85);
    // No full play spin: body yaw stays bounded.
    const yaw = Math.abs(builtinPetBodyYaw("pet.wave", 10, 0));
    expect(yaw).toBeLessThan(1.5);
  });

  it("maps pet.look_around onto observe lookSweep channel", () => {
    expect(builtinPetPose("pet.look_around", "neutral").lookAround).toBeGreaterThan(
      builtinPetPose("idle", "neutral").lookAround,
    );
    let maxHeadYaw = 0;
    for (let t = 0; t < 20; t += 0.25) {
      const sample = sampleBuiltinPetMotion("pet.look_around", "neutral", t, 0, 0, 1);
      maxHeadYaw = Math.max(maxHeadYaw, Math.abs(sample.headYaw));
    }
    expect(maxHeadYaw).toBeGreaterThan(0.12);
  });

  it("hops on pet.hop without linear body travel spin", () => {
    expect(builtinPetPose("pet.hop", "happy").hop).toBeGreaterThan(0.15);
    let maxRootY = -Infinity;
    let minRootY = Infinity;
    for (let t = 0; t < 3; t += 0.05) {
      const sample = sampleBuiltinPetMotion("pet.hop", "happy", t, 0, 0, 1);
      maxRootY = Math.max(maxRootY, sample.rootY);
      minRootY = Math.min(minRootY, sample.rootY);
    }
    expect(maxRootY - minRootY).toBeGreaterThan(0.08);
    // Hop uses vertical channel only — no play-style continuous spin.
    expect(Math.abs(builtinPetBodyYaw("pet.hop", Math.PI * 4, 0))).toBeLessThan(1);
  });
});

describe("lifeform perf emit contract", () => {
  it("keeps emit gate and summary helpers pure for the render loop", async () => {
    const {
      createLifeformPerfTracker,
      createPerfEmitGate,
      LIFEFORM_PERF_EVENT,
      LIFEFORM_PERF_EMIT_INTERVAL_MS,
      recordFrame,
      summarize,
    } = await import("./lifeformPerf");

    expect(LIFEFORM_PERF_EVENT).toBe("nimora:lifeform-perf");
    expect(LIFEFORM_PERF_EMIT_INTERVAL_MS).toBe(500);

    const tracker = createLifeformPerfTracker();
    const gate = createPerfEmitGate(500);
    for (let i = 0; i < 10; i += 1) {
      recordFrame(tracker, i * 16.67, 16.67);
    }
    const summary = summarize(tracker);
    expect(summary.sampleCount).toBe(10);
    expect(summary.fps).toBeGreaterThan(50);
    expect(gate.shouldEmit(0)).toBe(true);
    expect(gate.shouldEmit(200)).toBe(false);
    expect(gate.shouldEmit(500)).toBe(true);
  });
});
