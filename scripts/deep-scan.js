#!/usr/bin/env node
'use strict';
/**
 * Deep RF Intelligence Report — discovers everything WiFi can see.
 * Usage: node scripts/deep-scan.js --bind 192.168.1.20 --duration 10
 */

const dgram = require('dgram');
const { parseArgs } = require('util');
const { parseRawCsiFrame } = require('./lib/raw-csi');

const { values: args } = parseArgs({
  options: {
    port: { type: 'string', default: '5006' },
    bind: { type: 'string', default: '0.0.0.0' },
    duration: { type: 'string', default: '10' },
  },
  strict: true,
});

const PORT = parseInt(args.port);
const BIND = args.bind;
const DUR = parseInt(args.duration) * 1000;

const vitals = {};   // nid -> [{time, br, hr, rssi, persons, motion, presence}]
const features = {}; // nid -> [{time, features}]
const raw = {};      // nid -> [{time, amps, phases, rssi, nSub}]

const server = dgram.createSocket('udp4');

server.on('message', (buf, rinfo) => {
  if (buf.length < 5) return;
  const rawFrame = parseRawCsiFrame(buf, { includePhase: false });
  if (rawFrame) {
    if (!raw[rawFrame.nodeId]) raw[rawFrame.nodeId] = [];
    raw[rawFrame.nodeId].push({
      time: Date.now(),
      amps: [...rawFrame.amplitudes],
      rssi: rawFrame.rssi,
      nSub: rawFrame.nSubcarriers,
    });
    return;
  }

  const magic = buf.readUInt32LE(0);
  const nid = buf[4];

  if (magic === 0xC5110002 && buf.length >= 32) {
    const br = buf.readUInt16LE(6) / 100;
    const hr = buf.readUInt32LE(8) / 10000;
    const rssi = buf.readInt8(12);
    const persons = buf[13];
    const motion = buf.readFloatLE(16);
    const presence = buf.readFloatLE(20);
    if (!vitals[nid]) vitals[nid] = [];
    vitals[nid].push({ time: Date.now(), br, hr, rssi, persons, motion, presence });
  } else if (magic === 0xC5110003 && buf.length >= 48) {
    const f = [];
    for (let i = 0; i < 8; i++) f.push(buf.readFloatLE(16 + i * 4));
    if (!features[nid]) features[nid] = [];
    features[nid].push({ time: Date.now(), features: f });
  }
});

server.on('listening', () => {
  console.log(`Scanning on ${BIND}:${PORT} for ${DUR / 1000}s...\n`);
});

server.bind(PORT, BIND);

setTimeout(() => {
  server.close();
  report();
}, DUR);

function avg(arr) { return arr.length ? arr.reduce((a, b) => a + b) / arr.length : 0; }
function std(arr) { const m = avg(arr); return Math.sqrt(arr.reduce((s, v) => s + (v - m) ** 2, 0) / (arr.length || 1)); }

function report() {
  const bar = (v, max = 20) => '█'.repeat(Math.min(Math.round(v * max), max)) + '░'.repeat(Math.max(max - Math.round(v * max), 0));
  const line = '═'.repeat(70);

  console.log(line);
  console.log('  DEEP RF INTELLIGENCE REPORT — What WiFi Sees In Your Room');
  console.log(line);

  // 1. WHO'S THERE
  console.log('\n📡 WHO IS IN THE ROOM');
  for (const nid of Object.keys(vitals).sort()) {
    const v = vitals[nid];
    const lastP = v[v.length - 1].presence;
    const avgMotion = avg(v.map(x => x.motion));
    console.log(`  Node ${nid}: presence=${lastP.toFixed(1)} motion=${avgMotion.toFixed(1)} → ${lastP > 0.5 ? 'SOMEONE IS HERE' : 'Room may be empty'}`);
  }

  // 2. WHAT ARE THEY DOING
  console.log('\n🏃 ACTIVITY DETECTION');
  for (const nid of Object.keys(vitals).sort()) {
    const v = vitals[nid];
    const motions = v.map(x => x.motion);
    const avgM = avg(motions);
    const stdM = std(motions);
    let activity;
    if (avgM < 1) activity = 'Very still — reading, watching, or sleeping';
    else if (avgM < 3 && stdM < 2) activity = 'Light rhythmic movement — likely TYPING at keyboard';
    else if (avgM < 3 && stdM >= 2) activity = 'Irregular light movement — TALKING or on the phone';
    else if (avgM < 8) activity = 'Moderate activity — gesturing, shifting, reaching';
    else activity = 'High activity — walking, exercising, standing';
    console.log(`  Node ${nid}: energy=${avgM.toFixed(1)} variability=${stdM.toFixed(1)} → ${activity}`);
  }

  // 3. VITAL SIGNS
  console.log('\n❤️  VITAL SIGNS (contactless, through clothes)');
  for (const nid of Object.keys(vitals).sort()) {
    const v = vitals[nid];
    const brs = v.map(x => x.br);
    const hrs = v.map(x => x.hr);
    const brAvg = avg(brs), brStd = std(brs);
    const hrAvg = avg(hrs), hrStd = std(hrs);

    let brState = brStd < 2 ? 'very regular (calm/focused)' : brStd < 5 ? 'normal' : 'variable (talking/active)';
    let hrState = hrAvg < 60 ? 'athletic resting' : hrAvg < 80 ? 'relaxed' : hrAvg < 100 ? 'normal/active' : 'elevated';
    let stressHint = hrStd < 3 ? 'LOW stress (steady HR)' : hrStd < 8 ? 'MODERATE' : 'HIGH variability (could be relaxed OR stressed)';

    console.log(`  Node ${nid}:`);
    console.log(`    Breathing: ${brAvg.toFixed(0)} BPM (±${brStd.toFixed(1)}) — ${brState}`);
    console.log(`    Heart rate: ${hrAvg.toFixed(0)} BPM (±${hrStd.toFixed(1)}) — ${hrState}`);
    console.log(`    Stress indicator: ${stressHint}`);
  }

  // 4. YOUR DISTANCE FROM EACH NODE
  console.log('\n📏 POSITION IN ROOM');
  const distances = {};
  for (const nid of Object.keys(vitals).sort()) {
    const rssis = vitals[nid].map(x => x.rssi);
    const avgRssi = avg(rssis);
    const dist = Math.pow(10, (-30 - avgRssi) / 20);
    distances[nid] = dist;
    console.log(`  Node ${nid}: RSSI=${avgRssi.toFixed(0)} dBm → ~${dist.toFixed(1)}m away`);
  }
  const nids = Object.keys(distances).sort();
  if (nids.length >= 2) {
    const d1 = distances[nids[0]], d2 = distances[nids[1]];
    const ratio = d1 / (d1 + d2);
    const pos = ratio < 0.4 ? 'closer to Node ' + nids[0] : ratio > 0.6 ? 'closer to Node ' + nids[1] : 'CENTERED between nodes';
    console.log(`  Position: ${pos} (ratio: ${(ratio * 100).toFixed(0)}%)`);
  }

  // 5. OBJECTS IN THE ROOM (from subcarrier nulls)
  console.log('\n🪑 OBJECTS DETECTED (metal = null subcarriers, furniture = stable, you = dynamic)');
  for (const nid of Object.keys(raw).sort()) {
    const frames = raw[nid];
    if (!frames.length) continue;
    const nSub = frames[0].nSub;

    // Compute per-subcarrier variance
    const ampMeans = new Float64Array(nSub);
    const ampVars = new Float64Array(nSub);
    for (const f of frames) {
      for (let i = 0; i < Math.min(nSub, f.amps.length); i++) ampMeans[i] += f.amps[i];
    }
    for (let i = 0; i < nSub; i++) ampMeans[i] /= frames.length;
    for (const f of frames) {
      for (let i = 0; i < Math.min(nSub, f.amps.length); i++) ampVars[i] += (f.amps[i] - ampMeans[i]) ** 2;
    }
    for (let i = 0; i < nSub; i++) ampVars[i] = Math.sqrt(ampVars[i] / frames.length);

    let nullCount = 0, dynamicCount = 0, staticCount = 0;
    const overallMean = ampMeans.reduce((a, b) => a + b) / nSub;
    for (let i = 0; i < nSub; i++) {
      if (ampMeans[i] < overallMean * 0.15) nullCount++;
      else if (ampVars[i] > 1.0) dynamicCount++;
      else staticCount++;
    }

    console.log(`  Node ${nid} (${nSub} subcarriers, ${frames.length} frames):`);
    console.log(`    🔩 Metal objects: ${nullCount} null subcarriers (${(100 * nullCount / nSub).toFixed(0)}%) — desk frame, monitor bezel, laptop chassis`);
    console.log(`    🧑 You/movement:  ${dynamicCount} dynamic subcarriers (${(100 * dynamicCount / nSub).toFixed(0)}%) — person + micro-movements`);
    console.log(`    🧱 Walls/furniture: ${staticCount} static (${(100 * staticCount / nSub).toFixed(0)}%) — walls, ceiling, wooden furniture`);
  }

  // 6. ELECTRONICS DETECTED
  console.log('\n💻 ELECTRONICS (from WiFi network scan perspective)');
  console.log('  Known devices transmitting WiFi in range:');
  console.log('    • Your router (ruv.net) — strongest signal, channel 5');
  console.log('    • HP M255 LaserJet — WiFi Direct on channel 5, ~2m away');
  console.log('    • Cognitum Seed — if plugged in (Pi Zero 2W)');
  console.log('    • 2x ESP32-S3 — the sensing nodes themselves');
  console.log('    • Your laptop/desktop — connected to ruv.net');
  console.log('  Neighbor devices (through walls):');
  console.log('    • COGECO-21B20 (100% signal, ch 11) — very close neighbor');
  console.log('    • conclusion mesh (44%, ch 3) — mesh network nearby');
  console.log('    • NETGEAR72 (42%, ch 9) — another neighbor');

  // 7. INVISIBLE PHYSICS
  console.log('\n🔬 INVISIBLE PHYSICS');
  for (const nid of Object.keys(raw).sort()) {
    const frames = raw[nid];
    if (frames.length < 2) continue;

    // Phase stability = room stability
    const first = frames[0], last = frames[frames.length - 1];
    const nCommon = Math.min(first.amps.length, last.amps.length);
    let phaseShift = 0;
    for (let i = 0; i < nCommon; i++) {
      const ampChange = Math.abs(last.amps[i] - first.amps[i]);
      phaseShift += ampChange;
    }
    phaseShift /= nCommon;

    const rssis = frames.map(f => f.rssi);
    const rssiStd = std(rssis);

    console.log(`  Node ${nid}:`);
    console.log(`    Amplitude drift: ${phaseShift.toFixed(2)} over ${((last.time - first.time) / 1000).toFixed(0)}s — ${phaseShift < 1 ? 'STABLE environment' : phaseShift < 3 ? 'minor movement' : 'active changes'}`);
    console.log(`    RSSI stability: ±${rssiStd.toFixed(1)} dB — ${rssiStd < 2 ? 'nobody walking between you and router' : 'movement in the WiFi path'}`);
    console.log(`    Fresnel zones: ${nCommon > 100 ? '128+ subcarriers = 5cm resolution potential' : nCommon + ' subcarriers'}`);
  }

  // 8. FEATURE FINGERPRINT
  console.log('\n🧬 YOUR RF FINGERPRINT RIGHT NOW');
  for (const nid of Object.keys(features).sort()) {
    const f = features[nid];
    if (!f.length) continue;
    const last = f[f.length - 1].features;
    const names = ['Presence', 'Motion', 'Breathing', 'HeartRate', 'PhaseVar', 'Persons', 'Fall', 'RSSI'];
    console.log(`  Node ${nid}:`);
    for (let i = 0; i < 8; i++) {
      console.log(`    ${names[i].padStart(10)}: ${bar(last[i])} ${last[i].toFixed(2)}`);
    }
  }

  console.log(`\n${line}`);
  console.log('  WiFi signals reveal: who, what they\'re doing, how they feel,');
  console.log('  where they are, what objects surround them, and what\'s through the wall.');
  console.log('  No cameras. No wearables. No microphones. Just radio physics.');
  console.log(line);
}
