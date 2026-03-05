/**
 * PoseSystem -- Stateless pose keypoint generator for COCO 17-keypoint format.
 *
 * Keypoint indices:
 *   0:nose  1:left_eye  2:right_eye  3:left_ear  4:right_ear
 *   5:left_shoulder  6:right_shoulder  7:left_elbow  8:right_elbow
 *   9:left_wrist  10:right_wrist  11:left_hip  12:right_hip
 *   13:left_knee  14:right_knee  15:left_ankle  16:right_ankle
 *
 * Every public method is a pure function: parameters in, keypoint array out.
 */

export class PoseSystem {

  // ---- Entry point -------------------------------------------------------

  generateKeypoints(person, elapsed, breathPulse) {
    const pose = person.pose || 'standing';
    const pos = person.position || [0, 0, 0];
    const facing = person.facing || 0;
    const px = pos[0], pz = pos[2];
    const ms = person.motion_score || 0;
    const bp = breathPulse;

    let kps;
    switch (pose) {
      case 'lying':       kps = this.poseLying(px, pos[1] || 0, pz, elapsed, bp); break;
      case 'sitting':     kps = this.poseSitting(px, pz, elapsed, bp); break;
      case 'fallen':      kps = this.poseFallen(px, pz, elapsed); break;
      case 'falling':     kps = this.poseFalling(px, pz, elapsed, person.fallProgress || 0); break;
      case 'exercising':  kps = this.poseExercising(px, pz, elapsed, person.exerciseType, person.exerciseTime); break;
      case 'gesturing':   kps = this.poseGesturing(px, pz, elapsed, person.gestureType, person.gestureIntensity || 0); break;
      case 'crouching':   kps = this.poseCrouching(px, pz, elapsed, bp); break;
      case 'walking':     kps = this.poseWalking(px, pz, elapsed, ms, bp); break;
      case 'standing':
      default:            kps = this.poseStanding(px, pz, elapsed, ms, bp); break;
    }

    // Apply facing rotation
    if (Math.abs(facing) > 0.01) {
      this.rotateKps(kps, px, pz, facing);
    }
    return kps;
  }

  // ---- Rotation utility --------------------------------------------------

  rotateKps(kps, cx, cz, angle) {
    const cos = Math.cos(angle), sin = Math.sin(angle);
    for (const kp of kps) {
      const dx = kp[0] - cx, dz = kp[2] - cz;
      kp[0] = cx + dx * cos - dz * sin;
      kp[2] = cz + dx * sin + dz * cos;
    }
  }

  // ---- Standing ----------------------------------------------------------
  // Weight shift between feet, idle head look-around, breathing

  poseStanding(px, pz, elapsed, ms, bp) {
    // Slow weight shift side to side
    const weightShift = Math.sin(elapsed * 0.6) * 0.012;
    // Idle head look around
    const headTurn = Math.sin(elapsed * 0.3) * 0.015;
    const headTilt = Math.cos(elapsed * 0.25) * 0.008;
    // Slight sway from micro-balance adjustments
    const sway = Math.sin(elapsed * 0.8) * 0.005 + weightShift;
    // Knee bend alternation with weight shift
    const leftKneeBend = Math.max(0, Math.sin(elapsed * 0.6)) * 0.015;
    const rightKneeBend = Math.max(0, -Math.sin(elapsed * 0.6)) * 0.015;

    return [
      [px + sway + headTurn, 1.72 + bp + headTilt, pz],                        // 0 nose
      [px - 0.03 + sway + headTurn, 1.74 + bp + headTilt, pz - 0.02],          // 1 left eye
      [px + 0.03 + sway + headTurn, 1.74 + bp + headTilt, pz - 0.02],          // 2 right eye
      [px - 0.07 + headTurn * 0.5, 1.72 + bp, pz],                             // 3 left ear
      [px + 0.07 + headTurn * 0.5, 1.72 + bp, pz],                             // 4 right ear
      [px - 0.22 + weightShift * 0.3, 1.48 + bp, pz],                          // 5 left shoulder
      [px + 0.22 + weightShift * 0.3, 1.48 + bp, pz],                          // 6 right shoulder
      [px - 0.24 + weightShift * 0.2, 1.18 + bp, pz + 0.02],                   // 7 left elbow
      [px + 0.24 + weightShift * 0.2, 1.18 + bp, pz - 0.02],                   // 8 right elbow
      [px - 0.22 + weightShift * 0.15, 0.92 + bp, pz + 0.05],                  // 9 left wrist
      [px + 0.22 + weightShift * 0.15, 0.92 + bp, pz - 0.05],                  // 10 right wrist
      [px - 0.11 + weightShift * 0.5, 0.98 + bp, pz],                          // 11 left hip
      [px + 0.11 + weightShift * 0.5, 0.98 + bp, pz],                          // 12 right hip
      [px - 0.12 + weightShift * 0.3, 0.52 + leftKneeBend, pz],                // 13 left knee
      [px + 0.12 + weightShift * 0.3, 0.52 + rightKneeBend, pz],               // 14 right knee
      [px - 0.12 + weightShift * 0.4, 0.04, pz],                               // 15 left ankle
      [px + 0.12 + weightShift * 0.4, 0.04, pz],                               // 16 right ankle
    ];
  }

  // ---- Walking -----------------------------------------------------------
  // Torso rotation, head bob, natural arm pendulum with elbow bend

  poseWalking(px, pz, elapsed, ms, bp) {
    const speed = Math.min(ms / 100, 2.5);
    const wp = elapsed * speed * 1.8;
    const sFactor = Math.min(speed, 1);

    // Leg stride
    const legStride = Math.sin(wp) * 0.25 * sFactor;
    const legBack = Math.sin(wp + Math.PI) * 0.25 * sFactor;
    const kneeAmt = Math.abs(Math.sin(wp)) * 0.08;

    // Natural arm pendulum -- opposite to legs, with elbow bend
    const armPhase = Math.sin(wp);
    const armSwingL = -armPhase * 0.3 * sFactor;   // left arm opposite right leg
    const armSwingR = armPhase * 0.3 * sFactor;
    const elbowBendL = Math.max(0, -armPhase) * 0.12 * sFactor; // bend on backswing
    const elbowBendR = Math.max(0, armPhase) * 0.12 * sFactor;

    // Torso twist (shoulders rotate opposite to hips)
    const torsoTwist = Math.sin(wp) * 0.03 * sFactor;

    // Vertical bob (double frequency -- peak at mid-stance)
    const bob = Math.abs(Math.sin(wp)) * 0.025;

    // Head bob -- slight lag behind body
    const headBob = Math.abs(Math.sin(wp - 0.2)) * 0.015;
    const headLean = Math.sin(wp) * 0.008;

    return [
      [px + headLean, 1.72 + bp + bob + headBob, pz],                                // 0 nose
      [px - 0.03 + headLean, 1.74 + bp + bob + headBob, pz - 0.02],                  // 1 left eye
      [px + 0.03 + headLean, 1.74 + bp + bob + headBob, pz - 0.02],                  // 2 right eye
      [px - 0.07, 1.72 + bp + bob + headBob, pz],                                     // 3 left ear
      [px + 0.07, 1.72 + bp + bob + headBob, pz],                                     // 4 right ear
      [px - 0.22 - torsoTwist, 1.48 + bp + bob, pz],                                  // 5 left shoulder (twist)
      [px + 0.22 - torsoTwist, 1.48 + bp + bob, pz],                                  // 6 right shoulder
      [px - 0.28 + armSwingL * 0.3, 1.18 + bp + bob - elbowBendL, pz + armSwingL * 0.3],  // 7 left elbow
      [px + 0.28 + armSwingR * 0.3, 1.18 + bp + bob - elbowBendR, pz + armSwingR * 0.3],  // 8 right elbow
      [px - 0.26 + armSwingL * 0.6, 0.92 + bp + bob - elbowBendL * 1.5, pz + armSwingL * 0.5],  // 9 left wrist
      [px + 0.26 + armSwingR * 0.6, 0.92 + bp + bob - elbowBendR * 1.5, pz + armSwingR * 0.5],  // 10 right wrist
      [px - 0.11 + torsoTwist * 0.5, 0.98 + bp + bob, pz],                           // 11 left hip (counter-twist)
      [px + 0.11 + torsoTwist * 0.5, 0.98 + bp + bob, pz],                           // 12 right hip
      [px - 0.12 + legStride * 0.3, 0.52 + kneeAmt, pz + legStride],                 // 13 left knee
      [px + 0.12 + legBack * 0.3, 0.52 + kneeAmt, pz + legBack],                     // 14 right knee
      [px - 0.12 + legStride * 0.6, 0.04, pz + legStride * 1.5],                     // 15 left ankle
      [px + 0.12 + legBack * 0.6, 0.04, pz + legBack * 1.5],                         // 16 right ankle
    ];
  }

  // ---- Lying -------------------------------------------------------------
  // Subtle micro-movements, differentiate supine vs side-lying via elapsed hash

  poseLying(px, surfaceY, pz, elapsed, bp) {
    const y = (surfaceY || 0) + 0.2;
    const chest = bp * 0.015;

    // Micro-movements -- tiny random-feeling shifts (deterministic from elapsed)
    const microX = Math.sin(elapsed * 0.17) * 0.004;
    const microZ = Math.cos(elapsed * 0.13) * 0.003;
    const fingerTwitch = Math.sin(elapsed * 0.7) * 0.008;

    // Determine supine vs side-lying from a slow oscillation (stays one way for ~20s)
    const lyingMode = Math.sin(elapsed * 0.05);

    if (lyingMode > 0.3) {
      // Side-lying (on left side)
      const curl = Math.sin(elapsed * 0.1) * 0.02; // slight fetal curl
      return [
        [px - 0.72 + microX, y + 0.12, pz - 0.08],                     // 0 nose (turned)
        [px - 0.70, y + 0.14, pz - 0.10],                               // 1 left eye
        [px - 0.70, y + 0.16, pz - 0.06],                               // 2 right eye (up)
        [px - 0.76, y + 0.11, pz - 0.12],                               // 3 left ear (down)
        [px - 0.76, y + 0.14, pz - 0.04],                               // 4 right ear
        [px - 0.45, y + chest + 0.05, pz - 0.12],                       // 5 left shoulder (down)
        [px - 0.45, y + chest + 0.2, pz + 0.04],                        // 6 right shoulder (up)
        [px - 0.38, y + 0.02, pz - 0.28 + curl],                        // 7 left elbow
        [px - 0.35, y + 0.18, pz + 0.15 + fingerTwitch],                // 8 right elbow
        [px - 0.20, y - 0.01, pz - 0.30 + curl],                        // 9 left wrist
        [px - 0.18, y + 0.12, pz + 0.25 + fingerTwitch],                // 10 right wrist
        [px + 0.05 + microX, y + chest * 0.4 + 0.03, pz - 0.08],        // 11 left hip
        [px + 0.05 + microX, y + chest * 0.4 + 0.12, pz + 0.06],        // 12 right hip
        [px + 0.40 + curl * 2, y + 0.02, pz - 0.14 + curl],             // 13 left knee
        [px + 0.38 + curl * 2, y + 0.10, pz + 0.10 + curl],             // 14 right knee
        [px + 0.75, y - 0.01, pz - 0.12],                                // 15 left ankle
        [px + 0.72, y + 0.04, pz + 0.08],                                // 16 right ankle
      ];
    }

    // Supine (face up) -- default
    return [
      [px - 0.75 + microX, y + 0.08, pz + microZ],                     // 0 nose
      [px - 0.72, y + 0.1, pz - 0.02 + microZ],                        // 1 left eye
      [px - 0.72, y + 0.1, pz + 0.02 + microZ],                        // 2 right eye
      [px - 0.78, y + 0.08, pz - 0.05],                                 // 3 left ear
      [px - 0.78, y + 0.08, pz + 0.05],                                 // 4 right ear
      [px - 0.45, y + chest, pz - 0.18],                                // 5 left shoulder
      [px - 0.45, y + chest, pz + 0.18],                                // 6 right shoulder
      [px - 0.42, y, pz - 0.35 + fingerTwitch],                         // 7 left elbow
      [px - 0.42, y, pz + 0.35 - fingerTwitch],                         // 8 right elbow
      [px - 0.2, y - 0.02, pz - 0.38 + fingerTwitch],                   // 9 left wrist
      [px - 0.2, y - 0.02, pz + 0.38 - fingerTwitch],                   // 10 right wrist
      [px + 0.05 + microX, y + chest * 0.5, pz - 0.1],                  // 11 left hip
      [px + 0.05 + microX, y + chest * 0.5, pz + 0.1],                  // 12 right hip
      [px + 0.45, y, pz - 0.11],                                         // 13 left knee
      [px + 0.45, y, pz + 0.11],                                         // 14 right knee
      [px + 0.82, y - 0.02, pz - 0.1],                                   // 15 left ankle
      [px + 0.82, y - 0.02, pz + 0.1],                                   // 16 right ankle
    ];
  }

  // ---- Sitting -----------------------------------------------------------
  // Occasional fidget, breathing chest expansion, weight shift

  poseSitting(px, pz, elapsed, bp) {
    const sway = Math.sin(elapsed * 0.5) * 0.003;

    // Fidget: occasional hand movement (every ~6s a small gesture)
    const fidgetCycle = elapsed % 6.0;
    const fidgetActive = fidgetCycle > 5.2 && fidgetCycle < 5.8;
    const fidgetAmt = fidgetActive ? Math.sin((fidgetCycle - 5.2) * Math.PI / 0.6) * 0.06 : 0;

    // Weight shift side to side (slow)
    const weightShift = Math.sin(elapsed * 0.25) * 0.008;

    // Chest expansion from breathing
    const chestExpand = bp * 0.008;

    return [
      [px + sway + weightShift, 1.15 + bp, pz],                                      // 0 nose
      [px - 0.03 + sway + weightShift, 1.17 + bp, pz - 0.02],                        // 1 left eye
      [px + 0.03 + sway + weightShift, 1.17 + bp, pz - 0.02],                        // 2 right eye
      [px - 0.07 + weightShift, 1.15 + bp, pz],                                       // 3 left ear
      [px + 0.07 + weightShift, 1.15 + bp, pz],                                       // 4 right ear
      [px - 0.20 - chestExpand + weightShift, 0.95 + bp, pz],                         // 5 left shoulder
      [px + 0.20 + chestExpand + weightShift, 0.95 + bp, pz],                         // 6 right shoulder
      [px - 0.25 + weightShift, 0.72 + bp, pz + 0.08],                                // 7 left elbow
      [px + 0.25 + weightShift, 0.72 + bp, pz + 0.08],                                // 8 right elbow
      [px - 0.18 + fidgetAmt, 0.55 + fidgetAmt * 0.3, pz + 0.15],                    // 9 left wrist (fidgets)
      [px + 0.18, 0.55, pz + 0.15],                                                    // 10 right wrist
      [px - 0.11 + weightShift * 0.5, 0.48, pz + 0.02],                               // 11 left hip
      [px + 0.11 + weightShift * 0.5, 0.48, pz + 0.02],                               // 12 right hip
      [px - 0.12, 0.48, pz + 0.4],                                                     // 13 left knee
      [px + 0.12, 0.48, pz + 0.4],                                                     // 14 right knee
      [px - 0.12, 0.04, pz + 0.4],                                                     // 15 left ankle
      [px + 0.12, 0.04, pz + 0.4],                                                     // 16 right ankle
    ];
  }

  // ---- Fallen ------------------------------------------------------------
  // Occasional twitch/attempt to move, asymmetric breathing

  poseFallen(px, pz, elapsed) {
    // Irregular twitch -- sharper, less periodic
    const twitchArm = Math.sin(elapsed * 0.3) * 0.003 +
                      Math.sin(elapsed * 1.7) * 0.008 * Math.max(0, Math.sin(elapsed * 0.15));
    const twitchLeg = Math.cos(elapsed * 0.4) * 0.005 *
                      Math.max(0, Math.sin(elapsed * 0.2 + 1.0));

    // Asymmetric breathing (one side of chest rises more)
    const breathL = Math.sin(elapsed * 0.8) * 0.006;
    const breathR = Math.sin(elapsed * 0.8 + 0.3) * 0.004;

    // Attempt to move (slow reach every ~10s)
    const attemptCycle = elapsed % 10.0;
    const attempting = attemptCycle > 8.0 && attemptCycle < 9.5;
    const attemptAmt = attempting ? Math.sin((attemptCycle - 8.0) * Math.PI / 1.5) * 0.05 : 0;

    return [
      [px + 0.35, 0.12, pz + 0.15 + twitchArm],                        // 0 nose
      [px + 0.33, 0.14, pz + 0.13],                                      // 1 left eye
      [px + 0.37, 0.14, pz + 0.17],                                      // 2 right eye
      [px + 0.38, 0.11, pz + 0.1],                                       // 3 left ear
      [px + 0.38, 0.11, pz + 0.2],                                       // 4 right ear
      [px + 0.15, 0.15 + breathL, pz - 0.1],                             // 5 left shoulder
      [px + 0.15, 0.2 + breathR, pz + 0.25],                             // 6 right shoulder
      [px - 0.05, 0.08, pz - 0.25 + twitchArm],                          // 7 left elbow
      [px + 0.3, 0.22 + attemptAmt * 0.5, pz + 0.45 + attemptAmt],       // 8 right elbow (reaching)
      [px - 0.15, 0.05, pz - 0.3 + twitchArm * 1.5],                     // 9 left wrist
      [px + 0.4, 0.15 + attemptAmt, pz + 0.5 + attemptAmt * 1.5],        // 10 right wrist (reaching)
      [px - 0.05, 0.12, pz - 0.05],                                       // 11 left hip
      [px - 0.05, 0.12, pz + 0.15],                                       // 12 right hip
      [px - 0.2, 0.08 + twitchLeg, pz - 0.3],                            // 13 left knee
      [px - 0.15, 0.15, pz + 0.35 + twitchLeg],                          // 14 right knee
      [px - 0.35, 0.04, pz - 0.2],                                        // 15 left ankle
      [px - 0.3, 0.04, pz + 0.5],                                         // 16 right ankle
    ];
  }

  // ---- Falling -----------------------------------------------------------
  // Flailing arms, head snap, non-linear easing (cubic ease-in)

  poseFalling(px, pz, elapsed, progress) {
    const standing = this.poseStanding(px, pz, elapsed, 0, 0);
    const fallen = this.poseFallen(px, pz, elapsed);

    // Cubic ease-in for realistic acceleration
    const t = progress * progress * progress;

    // Arm flailing -- sinusoidal perturbation that peaks mid-fall then diminishes
    const flailIntensity = Math.sin(progress * Math.PI) * 0.15;
    const flailL = Math.sin(elapsed * 8 + progress * 5) * flailIntensity;
    const flailR = Math.cos(elapsed * 8 + progress * 5) * flailIntensity;

    // Head snaps back early in the fall
    const headSnap = progress < 0.4 ? Math.sin(progress * Math.PI / 0.4) * 0.06 : 0;

    const kps = [];
    for (let i = 0; i < 17; i++) {
      kps.push([
        standing[i][0] * (1 - t) + fallen[i][0] * t,
        standing[i][1] * (1 - t) + fallen[i][1] * t,
        standing[i][2] * (1 - t) + fallen[i][2] * t,
      ]);
    }

    // Apply head snap (tilt backward)
    kps[0][1] += headSnap;
    kps[1][1] += headSnap * 0.9;
    kps[2][1] += headSnap * 0.9;

    // Apply arm flailing
    kps[7][0] += flailL;  kps[7][2] += flailL * 0.5;   // left elbow
    kps[8][0] += flailR;  kps[8][2] -= flailR * 0.5;   // right elbow
    kps[9][0] += flailL * 1.5;  kps[9][2] += flailL;   // left wrist
    kps[10][0] += flailR * 1.5; kps[10][2] -= flailR;   // right wrist

    return kps;
  }

  // ---- Exercising --------------------------------------------------------

  poseExercising(px, pz, elapsed, exerciseType, exerciseTime) {
    const et = exerciseTime || elapsed;

    if (exerciseType === 'squats') {
      return this._poseSquats(px, pz, et);
    }
    return this._poseJumpingJacks(px, pz, et);
  }

  // Squats: forward lean, hip hinge, arm counterbalance, depth variation

  _poseSquats(px, pz, et) {
    const rawPhase = (Math.sin(et * 2.5) + 1) / 2; // 0=up, 1=down
    // Depth variation -- every other rep is shallower
    const repIndex = Math.floor(et * 2.5 / Math.PI);
    const depthMod = (repIndex % 2 === 0) ? 1.0 : 0.7;
    const phase = rawPhase * depthMod;

    const squat = phase * 0.5;
    const armFwd = phase * 0.4;
    // Forward lean increases with squat depth
    const forwardLean = phase * 0.08;
    // Hip hinge -- hips push back
    const hipBack = phase * 0.12;

    return [
      [px + forwardLean * 0.3, 1.72 - squat, pz + forwardLean],                          // 0 nose
      [px - 0.03 + forwardLean * 0.3, 1.74 - squat, pz - 0.02 + forwardLean],            // 1 left eye
      [px + 0.03 + forwardLean * 0.3, 1.74 - squat, pz - 0.02 + forwardLean],            // 2 right eye
      [px - 0.07, 1.72 - squat, pz + forwardLean * 0.8],                                  // 3 left ear
      [px + 0.07, 1.72 - squat, pz + forwardLean * 0.8],                                  // 4 right ear
      [px - 0.22, 1.48 - squat + forwardLean * 0.2, pz + forwardLean * 0.5],              // 5 left shoulder
      [px + 0.22, 1.48 - squat + forwardLean * 0.2, pz + forwardLean * 0.5],              // 6 right shoulder
      [px - 0.22, 1.25 - squat * 0.7, pz + armFwd],                                       // 7 left elbow
      [px + 0.22, 1.25 - squat * 0.7, pz + armFwd],                                       // 8 right elbow
      [px - 0.22, 1.05 - squat * 0.5, pz + armFwd * 1.5],                                 // 9 left wrist (counterbalance)
      [px + 0.22, 1.05 - squat * 0.5, pz + armFwd * 1.5],                                 // 10 right wrist
      [px - 0.11, 0.98 - squat * 0.3, pz - hipBack],                                      // 11 left hip (pushed back)
      [px + 0.11, 0.98 - squat * 0.3, pz - hipBack],                                      // 12 right hip
      [px - 0.15, 0.52 - squat * 0.1, pz + squat * 0.3],                                  // 13 left knee
      [px + 0.15, 0.52 - squat * 0.1, pz + squat * 0.3],                                  // 14 right knee
      [px - 0.13, 0.04, pz + 0.05],                                                        // 15 left ankle
      [px + 0.13, 0.04, pz + 0.05],                                                        // 16 right ankle
    ];
  }

  // Jumping jacks: full arm arc, hip sway, landing impact

  _poseJumpingJacks(px, pz, et) {
    const rawPhase = (Math.sin(et * 3) + 1) / 2; // 0=closed, 1=open
    const phase = rawPhase;

    // Full arm arc -- from sides to overhead in a smooth arc
    const armAngle = phase * Math.PI * 0.85; // 0 to ~153 degrees
    const armX = Math.sin(armAngle) * 0.55;  // lateral spread
    const armY = Math.cos(armAngle) * 0.55;  // vertical component

    const legSpread = phase * 0.25;
    // Landing impact -- brief compression at bottom of cycle
    const impact = Math.max(0, -Math.sin(et * 3)) * 0.03;
    const jump = Math.max(0, Math.sin(et * 3)) * 0.06;
    // Hip sway at apex
    const hipSway = Math.sin(et * 3) * 0.015;

    return [
      [px, 1.72 + jump - impact, pz],                                                     // 0 nose
      [px - 0.03, 1.74 + jump - impact, pz - 0.02],                                       // 1 left eye
      [px + 0.03, 1.74 + jump - impact, pz - 0.02],                                       // 2 right eye
      [px - 0.07, 1.72 + jump - impact, pz],                                               // 3 left ear
      [px + 0.07, 1.72 + jump - impact, pz],                                               // 4 right ear
      [px - 0.22, 1.48 + jump - impact, pz],                                               // 5 left shoulder
      [px + 0.22, 1.48 + jump - impact, pz],                                               // 6 right shoulder
      [px - 0.22 - armX * 0.6, 1.48 - armY * 0.3 + jump, pz],                             // 7 left elbow (arc)
      [px + 0.22 + armX * 0.6, 1.48 - armY * 0.3 + jump, pz],                             // 8 right elbow
      [px - 0.22 - armX, 1.48 - armY + 0.55 + jump, pz],                                  // 9 left wrist (arc)
      [px + 0.22 + armX, 1.48 - armY + 0.55 + jump, pz],                                  // 10 right wrist
      [px - 0.11 + hipSway, 0.98 + jump - impact, pz],                                    // 11 left hip
      [px + 0.11 + hipSway, 0.98 + jump - impact, pz],                                    // 12 right hip
      [px - 0.12 - legSpread, 0.52 + jump * 0.5 - impact * 0.5, pz],                      // 13 left knee
      [px + 0.12 + legSpread, 0.52 + jump * 0.5 - impact * 0.5, pz],                      // 14 right knee
      [px - 0.13 - legSpread * 1.3, 0.04 - impact * 0.3, pz],                             // 15 left ankle
      [px + 0.13 + legSpread * 1.3, 0.04 - impact * 0.3, pz],                             // 16 right ankle
    ];
  }

  // ---- Gesturing ---------------------------------------------------------

  poseGesturing(px, pz, elapsed, gestureType, intensity) {
    const base = this.poseStanding(px, pz, elapsed, 0, 0);
    if (intensity <= 0) return base;
    const gt = elapsed;

    switch (gestureType) {
      case 'wave':
        return this._gestureWave(base, px, pz, gt, intensity);
      case 'swipe_left':
        return this._gestureSwipe(base, px, pz, gt, intensity);
      case 'circle':
        return this._gestureCircle(base, px, pz, gt, intensity);
      case 'point':
        return this._gesturePoint(base, px, pz, gt, intensity);
      default:
        return base;
    }
  }

  // Wave: fluid hand oscillation, elbow pivot, slight shoulder raise

  _gestureWave(base, px, pz, gt, intensity) {
    const wave = Math.sin(gt * 6) * 0.15 * intensity;
    const waveSmooth = Math.sin(gt * 6 + 0.3) * 0.08 * intensity; // secondary harmonic
    const shoulderRaise = 0.04 * intensity;
    const elbowPivot = Math.sin(gt * 3) * 0.03 * intensity;

    // Shoulder rises slightly during wave
    base[6][1] += shoulderRaise;
    // Elbow raised and pivoting
    base[8] = [
      px + 0.32 + elbowPivot,
      1.55 * intensity + 1.18 * (1 - intensity) + shoulderRaise,
      pz + 0.05,
    ];
    // Wrist oscillates fluidly
    base[10] = [
      px + 0.32 + wave + waveSmooth * 0.3,
      1.7 * intensity + 0.92 * (1 - intensity) + shoulderRaise,
      pz + 0.08 + waveSmooth,
    ];
    // Slight body lean away from waving arm
    base[0][0] -= 0.01 * intensity;
    base[5][0] -= 0.008 * intensity;
    return base;
  }

  // Swipe: full body rotation follow-through, arm extension

  _gestureSwipe(base, px, pz, gt, intensity) {
    const sweep = Math.sin(gt * 2) * intensity;
    // Body rotation follows the arm
    const bodyRotation = sweep * 0.04;
    const shoulderTwist = sweep * 0.025;

    // Upper body rotates
    for (let i = 0; i <= 4; i++) base[i][0] += bodyRotation * 0.5;
    base[5][0] -= shoulderTwist;
    base[6][0] += shoulderTwist;

    // Arm extends fully during swipe
    base[8] = [px + 0.15 + sweep * 0.4, 1.3, pz + 0.3];
    base[10] = [px - 0.1 + sweep * 0.6, 1.3, pz + 0.55];

    // Hip counter-rotation
    base[11][0] += bodyRotation * -0.2;
    base[12][0] += bodyRotation * -0.2;
    return base;
  }

  // Circle: smooth circular motion with forearm rotation

  _gestureCircle(base, px, pz, gt, intensity) {
    const angle = gt * 2.5;
    const radius = 0.25 * intensity;
    const cx = Math.cos(angle) * radius;
    const cy = Math.sin(angle) * radius;
    // Forearm rotation -- wrist traces a smaller secondary circle
    const forearmAngle = angle * 1.5;
    const forearmR = 0.06 * intensity;

    base[8] = [
      px + 0.3 + cx * 0.5,
      1.3 + cy * 0.5,
      pz + 0.2 + Math.sin(angle) * 0.05,
    ];
    base[10] = [
      px + 0.3 + cx + Math.cos(forearmAngle) * forearmR,
      1.3 + cy + Math.sin(forearmAngle) * forearmR,
      pz + 0.35 + Math.sin(angle) * 0.08,
    ];
    // Slight shoulder movement following arm
    base[6][0] += cx * 0.08;
    base[6][1] += cy * 0.04;
    return base;
  }

  // Point: extended index finger simulation with arm sway

  _gesturePoint(base, px, pz, gt, intensity) {
    const point = intensity;
    // Slight arm sway -- breathing/holding still
    const sway = Math.sin(gt * 1.5) * 0.01 * intensity;
    const vertSway = Math.cos(gt * 1.2) * 0.008 * intensity;

    base[8] = [px + 0.15 + sway, 1.35 + vertSway, pz + 0.35 * point];
    base[10] = [px + 0.08 + sway * 0.5, 1.38 + vertSway * 0.5, pz + 0.70 * point];

    // Lean slightly toward point direction
    base[0][2] += 0.02 * point;
    base[5][2] += 0.01 * point;
    base[6][2] += 0.01 * point;
    return base;
  }

  // ---- Crouching ---------------------------------------------------------
  // Stealth-crawl option, weight transfer between legs

  poseCrouching(px, pz, elapsed, bp) {
    const sway = Math.sin(elapsed * 1.5) * 0.005;

    // Weight transfer between legs (slow rocking)
    const weightTransfer = Math.sin(elapsed * 0.8) * 0.025;
    const leftDown = Math.max(0, weightTransfer) * 0.03;
    const rightDown = Math.max(0, -weightTransfer) * 0.03;

    // Stealth-crawl micro-movement (slow forward creep every ~4s)
    const crawlCycle = elapsed % 4.0;
    const crawlActive = crawlCycle > 3.0;
    const crawlAmt = crawlActive ? Math.sin((crawlCycle - 3.0) * Math.PI) * 0.02 : 0;

    // Arms adjust for balance during weight transfer
    const armBalance = weightTransfer * 0.3;

    return [
      [px + sway, 1.05 + bp, pz + 0.15 + crawlAmt],                             // 0 nose
      [px - 0.03, 1.07 + bp, pz + 0.13 + crawlAmt],                              // 1 left eye
      [px + 0.03, 1.07 + bp, pz + 0.13 + crawlAmt],                              // 2 right eye
      [px - 0.07, 1.05 + bp, pz + 0.12 + crawlAmt],                              // 3 left ear
      [px + 0.07, 1.05 + bp, pz + 0.12 + crawlAmt],                              // 4 right ear
      [px - 0.22, 0.88 + bp, pz + 0.05],                                          // 5 left shoulder
      [px + 0.22, 0.88 + bp, pz + 0.05],                                          // 6 right shoulder
      [px - 0.28 - armBalance, 0.65 + bp, pz + 0.15 + crawlAmt * 0.5],           // 7 left elbow
      [px + 0.28 + armBalance, 0.65 + bp, pz + 0.15 + crawlAmt * 0.5],           // 8 right elbow
      [px - 0.22 - armBalance * 0.5, 0.48, pz + 0.2 + crawlAmt],                 // 9 left wrist
      [px + 0.22 + armBalance * 0.5, 0.48, pz + 0.2 + crawlAmt],                 // 10 right wrist
      [px - 0.12 + weightTransfer, 0.42, pz - 0.05],                              // 11 left hip
      [px + 0.12 + weightTransfer, 0.42, pz - 0.05],                              // 12 right hip
      [px - 0.15 + weightTransfer * 0.5, 0.35 - leftDown, pz + 0.25],            // 13 left knee
      [px + 0.15 + weightTransfer * 0.5, 0.35 - rightDown, pz + 0.25],           // 14 right knee
      [px - 0.13, 0.04, pz + 0.1],                                                 // 15 left ankle
      [px + 0.13, 0.04, pz + 0.1],                                                 // 16 right ankle
    ];
  }
}
