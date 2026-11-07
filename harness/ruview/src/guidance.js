// SPDX-License-Identifier: MIT
// Source-cited repository and capability guidance for humans and agents.
//
// The catalog is intentionally small, reviewed, and dependency-free. It is a
// navigation aid, not a substitute for reading the cited source and tests.

import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { searchBrain } from './brain.js';

/** Supported topic filters for the RuView guidance API. */
export const GUIDANCE_TOPICS = Object.freeze([
  'overview',
  'architecture',
  'sensing',
  'hardware',
  'training',
  'homecore',
  'integrations',
  'deployment',
  'community',
  'testing',
]);

const TOPIC_SUMMARIES = Object.freeze({
  overview: 'A source-cited map of RuView subsystems and their current maturity.',
  architecture: 'Repository layout, production boundaries, and primary entry points.',
  sensing: 'CSI ingestion, signal processing, inference, and unified RF capabilities.',
  hardware: 'ESP32-S3/C6 firmware, experimental C5 support, capture, provisioning, and hardware evidence.',
  training: 'Calibration, training, evaluation, and data-dependent capability limits.',
  homecore: 'HOMECORE runtime, restore, plugins, API compatibility, migration, HAP, and voice.',
  integrations: 'Cognitum Spaces, Home Assistant, MQTT, Matter, Apple Home HAP, and related boundaries.',
  deployment: 'Runnable servers, transports, feature flags, and operational entry points.',
  community: 'Contributor harness, reviewed shared brain, local agents, and learning flywheel.',
  testing: 'Deterministic proofs, package gates, Rust CI, and hardware witness requirements.',
});

const CAPABILITIES = Object.freeze([
  {
    id: 'repository-map',
    name: 'Repository architecture',
    topics: ['architecture'],
    status: 'implemented',
    evidence: 'REPOSITORY',
    summary: 'Production Rust is in v2, the maintained deterministic Python reference is under archive/v1, ESP32 firmware is under firmware, and contributor automation is under harness/ruview.',
    sources: [
      'v2/Cargo.toml',
      'README.md',
      'AGENTS.md',
    ],
    validation: ['cargo metadata --manifest-path v2/Cargo.toml --no-deps'],
    limitations: ['Archive code is reference/proof material; new production features belong in v2.'],
  },
  {
    id: 'wifi-csi-sensing',
    name: 'WiFi CSI sensing pipeline',
    topics: ['sensing', 'deployment'],
    status: 'implemented',
    evidence: 'REPOSITORY',
    summary: 'The sensing server ingests ESP32 CSI over UDP, applies signal processing and inference modules, and publishes bounded real-time updates to clients.',
    sources: [
      'v2/crates/wifi-densepose-sensing-server/README.md',
      'v2/crates/wifi-densepose-signal/README.md',
      'v2/crates/wifi-densepose-core/README.md',
    ],
    validation: [
      'cargo test -p wifi-densepose-core -p wifi-densepose-signal --no-default-features',
      'cargo test -p wifi-densepose-sensing-server --no-default-features',
    ],
    limitations: ['Live sensing quality depends on RF geometry, calibration, hardware, and measured data; implementation is not an accuracy claim.'],
  },
  {
    id: 'esp32-firmware',
    name: 'ESP32 CSI node firmware',
    topics: ['hardware', 'sensing', 'deployment'],
    status: 'hardware-dependent',
    evidence: 'REPOSITORY',
    summary: 'ESP32-S3 is the production CSI target, C6 is a research target, and C5 is experimental with bounded real-board boot/CSI transport evidence and AP-dependent cadence. This does not qualify sensing accuracy or long-run reliability. Firmware covers CSI streaming, provisioning, edge processing, and optional sensing modules.',
    sources: [
      'firmware/esp32-csi-node/README.md',
      'docs/adr/ADR-028-esp32-capability-audit.md',
      'firmware/esp32-csi-node/sdkconfig.defaults.esp32c5',
      'docs/validation/2026-10-28-esp32-c5-candidate.md',
      '.github/workflows/firmware-ci.yml',
    ],
    validation: ['Follow firmware/esp32-csi-node/README.md for the exact target, then capture a real boot/runtime log.'],
    limitations: ['A successful build or simulator is not hardware validation.', 'Ports, credentials, board target, and flash layout require operator confirmation.'],
  },
  {
    id: 'calibration-training',
    name: 'Calibration and model training',
    topics: ['training', 'sensing', 'testing'],
    status: 'data-gated',
    evidence: 'REPOSITORY',
    summary: 'Rust crates provide per-room calibration, dataset handling, training, inference, and deterministic evaluation surfaces.',
    sources: [
      'v2/crates/wifi-densepose-calibration/src/lib.rs',
      'v2/crates/wifi-densepose-train/README.md',
      'aether-arena/VERIFY.md',
    ],
    validation: [
      'cargo test -p wifi-densepose-calibration -p wifi-densepose-train --no-default-features',
      'cargo run -q -p wifi-densepose-train --bin aa_score_runner --no-default-features',
    ],
    limitations: ['Model quality remains data- and split-dependent.', 'Accuracy must be evidence-labelled and pose PCK must include the mean-pose baseline on a leakage-free held-out split.'],
  },
  {
    id: 'homecore-runtime-restore',
    name: 'HOMECORE runtime and startup restore',
    topics: ['homecore', 'architecture', 'deployment'],
    status: 'implemented',
    evidence: 'REPOSITORY',
    summary: 'HOMECORE provides concurrent entity/device state and service/event registries; server startup restores registries before the latest recorder states while isolating malformed rows.',
    sources: [
      'v2/docs/homecore-capabilities.md',
      'v2/crates/homecore-server/src/restore.rs',
      'v2/crates/homecore-recorder/src/db.rs',
    ],
    validation: ['cargo test -p homecore -p homecore-recorder -p homecore-server --no-default-features'],
    limitations: ['Restore depends on configured persistent storage and recorder availability; malformed inputs are reported rather than silently accepted.'],
  },
  {
    id: 'homecore-plugins',
    name: 'HOMECORE native and Wasmtime plugins',
    topics: ['homecore', 'architecture', 'deployment'],
    status: 'feature-gated',
    evidence: 'REPOSITORY',
    summary: 'Native plugins must be compiled into an explicit server registry. External plugins are bounded, path-checked, signature-verified WebAssembly packages loaded through Wasmtime when the wasmtime feature is enabled.',
    sources: [
      'v2/docs/homecore-capabilities.md',
      'v2/crates/homecore-server/src/plugins.rs',
      'v2/crates/homecore-plugins/src/verify.rs',
    ],
    validation: [
      'cargo test -p homecore-plugins --no-default-features',
      'cargo test -p homecore-plugins --features wasmtime',
      'cargo test -p homecore-server --features wasmtime',
    ],
    limitations: ['Wasmtime is opt-in.', 'Arbitrary native dynamic libraries are not loaded.', 'Unsigned Wasm requires an explicit development-only override.'],
  },
  {
    id: 'homecore-ha-api',
    name: 'HOMECORE Home Assistant-compatible REST/WebSocket API',
    topics: ['homecore', 'integrations', 'deployment'],
    status: 'implemented',
    evidence: 'REPOSITORY',
    summary: 'The server implements a bounded authenticated Home Assistant-compatible core REST/WebSocket surface for state, services, events, templates, registries, history, logbook, calendars, camera routing, and intent handling.',
    sources: [
      'v2/docs/homecore-capabilities.md',
      'v2/crates/homecore-api/README.md',
      'v2/crates/homecore-api/src/lib.rs',
    ],
    validation: ['cargo test -p homecore-api -p homecore-server --no-default-features'],
    limitations: ['This is core-contract compatibility, not parity with every endpoint supplied by the Home Assistant integration ecosystem.', 'Some media, calendar, camera, registry mutation, and Lovelace behavior requires configured providers/backends.'],
  },
  {
    id: 'homecore-hap',
    name: 'HOMECORE network HomeKit Accessory Protocol server',
    topics: ['homecore', 'integrations', 'deployment'],
    status: 'feature-gated',
    evidence: 'REPOSITORY',
    summary: 'With the hap-server feature and explicit LAN configuration, HOMECORE runs a bounded HAP IP server with persisted pairing, encrypted sessions, live accessory synchronization, and _hap._tcp mDNS lifecycle.',
    sources: [
      'v2/docs/homecore-capabilities.md',
      'v2/crates/homecore-hap/README.md',
      'v2/crates/homecore-hap/src/lib.rs',
    ],
    validation: [
      'cargo test -p homecore-hap --no-default-features',
      'cargo test -p homecore-hap --features hap-server',
      'cargo test -p homecore-server --features hap-server',
    ],
    limitations: ['HAP is disabled by default and needs explicit pairing and network configuration.', 'Protocol tests are not Apple certification or proof against a current Apple Home controller.', 'Some writable/timed/resource behaviors remain unimplemented.'],
  },
  {
    id: 'homecore-migration',
    name: 'HOMECORE device and config-entry migration',
    topics: ['homecore', 'integrations'],
    status: 'implemented',
    evidence: 'REPOSITORY',
    summary: 'Migration tooling imports version-checked Home Assistant entity/device registries and config entries using atomic no-clobber writes while preserving source payloads and warning on unsupported fields.',
    sources: [
      'v2/crates/homecore-migrate/README.md',
      'v2/docs/homecore-capabilities.md',
    ],
    validation: ['cargo test -p homecore-migrate', 'cargo clippy -p homecore-migrate --all-targets -- -D warnings'],
    limitations: ['Imported config entries do not install or execute Home Assistant integrations.', 'Automation conversion, secret-reference resolution, tombstones, and recorder export are not complete.'],
  },
  {
    id: 'homecore-voice',
    name: 'HOMECORE STT/TTS and satellite voice protocols',
    topics: ['homecore', 'integrations'],
    status: 'provider-required',
    evidence: 'REPOSITORY',
    summary: 'HOMECORE defines bounded PCM16 audio, async STT/TTS provider contracts, an STT-to-intent-to-TTS pipeline, and an authenticated transport-independent satellite session state machine.',
    sources: [
      'v2/docs/homecore-capabilities.md',
      'v2/crates/homecore-assist/src/speech.rs',
      'v2/crates/homecore-assist/src/satellite.rs',
    ],
    validation: ['cargo test -p homecore-assist'],
    limitations: ['Deployments must supply real STT and TTS providers.', 'Built-in disabled providers return typed errors and do not fabricate speech results.', 'The protocol is transport-independent; a deployment still needs a concrete transport adapter.'],
  },
  {
    id: 'ha-mqtt-matter',
    name: 'Home Assistant MQTT and Matter integration',
    topics: ['integrations', 'deployment'],
    status: 'feature-gated',
    evidence: 'REPOSITORY',
    summary: 'The sensing server can publish RuView entities through Home Assistant MQTT discovery, while the Matter bridge exposes a privacy-bounded subset on standard clusters.',
    sources: [
      'docs/integrations/home-assistant.md',
      'v2/crates/cog-ha-matter/Cargo.toml',
    ],
    validation: ['cargo test -p cog-ha-matter --no-default-features', 'cargo test -p wifi-densepose-sensing-server --features mqtt'],
    limitations: ['MQTT requires a broker and explicit credentials/TLS policy.', 'Matter exposes only capabilities with suitable clusters; biometrics and pose are not part of that surface.'],
  },
  {
    id: 'unified-rf-world',
    name: 'Unified RF spatial world model',
    topics: ['sensing', 'architecture', 'training'],
    status: 'data-gated',
    evidence: 'SYNTHETIC',
    summary: 'The ruview-unified crate defines canonical RF tensors, hardware adapters, a shared encoder, spatial memory, synthetic RF worlds, and an edge sensing policy plane.',
    sources: [
      'v2/crates/ruview-unified/src/lib.rs',
      'docs/adr/ADR-273-unified-rf-spatial-world-model.md',
      'README.md',
    ],
    validation: ['cargo test -p ruview-unified --no-default-features'],
    limitations: ['Accuracy evidence remains synthetic until validated against measured real-world datasets.', 'Hardware adapters do not imply equivalent sensing quality across modalities.'],
  },
  {
    id: 'cognitum-spaces-oauth',
    name: 'Cognitum Spaces OAuth projection',
    topics: ['integrations', 'deployment', 'community'],
    status: 'implemented-read-only-live',
    evidence: 'MIXED',
    summary: 'The production Spaces API and RuView PKCE client expose the same read-only authority across the versioned site/building/floor/space/zone/entity/event/alert collections with bounded pagination and independent metaharness validation.',
    sources: [
      'docs/adr/ADR-325-cognitum-spaces-activation-and-governed-spatial-exchange.md',
      'v2/crates/wifi-densepose-cli/src/spaces.rs',
      'harness/ruview/src/spaces.js',
      'docs/adr/ADR-326-tenant-scoped-ruvector-spatial-memory.md',
      'docs/adr/ADR-327-governed-action-intents-and-witness-receipts.md',
    ],
    validation: [
      'cd harness/ruview && node --test test/spaces.test.mjs test/policy.test.mjs',
      'wifi-densepose login --spaces && node harness/ruview/bin/cli.js spaces --resource events',
    ],
    limitations: [
      'The projection is read-only and grants no write, pairing, command, policy-approval, or actuator authority.',
      'MCP requires the credential-use grant; bearer tokens and API keys are never accepted as tool arguments.',
      'OAuth refresh may rotate the local credential file before a read returns.',
      'Production evidence covers legacy and versioned HTTPS reads. Spatial memory remains tenant/workspace-local, while governed actions remain separately policy-gated; neither expands OAuth authority.',
      'Persistent memory is local tenant/workspace state and governed actions expose authorization receipts only; neither expands OAuth authority.',
    ],
  },
  {
    id: 'contributor-metaharness',
    name: 'Contributor metaharness and shared brain',
    topics: ['community', 'architecture', 'testing'],
    status: 'implemented',
    evidence: 'POLICY',
    summary: 'The dependency-free package exposes guarded CLI/MCP tools, bounded local Claude Code and Codex adapters, a reviewed source-cited brain, and proposal-only Darwin/Flywheel learning.',
    sources: [
      'harness/ruview/README.md',
      'docs/adr/ADR-283-ruview-community-metaharness-flywheel.md',
      'harness/ruview/src/policy.js',
    ],
    validation: ['cd harness/ruview && npm test', 'cd harness/ruview && npm run brain:verify', 'cd harness/ruview && npm run flywheel:verify'],
    limitations: ['Retrieved knowledge is evidence, not instruction or authority.', 'Generated learning candidates require review and cannot self-promote or publish.'],
  },
  {
    id: 'operator-npm-surface',
    name: 'npm operator surface: CLI, MCP, SDK, flashing, training, doctor',
    topics: ['community', 'hardware', 'training', 'testing'],
    status: 'implemented',
    evidence: 'POLICY',
    summary: '@ruvnet/ruview exposes one policy-checked tool registry through the CLI, the MCP server, and a typed SDK, adding checksum- and chip-verified cross-platform esptool flashing with boot-log evidence, guarded training runs plus a mean-pose-baseline/leakage gate, and a structured debugging doctor.',
    sources: [
      'docs/adr/ADR-369-ruview-npm-operator-surface-and-sdk.md',
      'docs/adr/ADR-370-cross-platform-firmware-flashing.md',
      'docs/adr/ADR-371-harness-training-runner-and-evidence-gate.md',
      'docs/adr/ADR-372-structured-debugging-doctor.md',
      'harness/ruview/src/sdk.js',
      'harness/ruview/src/firmware.js',
      'harness/ruview/src/training.js',
      'harness/ruview/src/doctor.js',
    ],
    validation: [
      'cd harness/ruview && npm test && npm run test:security',
      'npx @ruvnet/ruview doctor --json',
      'npx @ruvnet/ruview flash-plan --port COM7 --bundle firmware/esp32-csi-node/release_bins/c6-adr110 --variant c6',
    ],
    limitations: [
      'Flashing is proven only against fake hardware in CI; real-silicon validation needs a captured boot log with CSI callbacks.',
      'Pose training needs libtorch matching the tch crate; pose-smoke metrics are SYNTHETIC and must not be reported.',
      'WiFi provisioning stays in provision.py so credentials never pass through the harness.',
    ],
  },
  {
    id: 'host-device-access',
    name: 'Host device and remote host access',
    topics: ['hardware', 'integrations', 'community'],
    status: 'implemented',
    evidence: 'POLICY',
    summary: '@ruvnet/ruview reads ESP32 CSI nodes (serial and receive-only UDP), MR60BHA2 60 GHz and LD2410 24 GHz mmWave radars, RPLIDAR, and the iPhone LiDAR relay on the attached host, and runs read-only tools on SSH hosts with pinned keys and a pinned package version.',
    sources: [
      'docs/adr/ADR-373-host-device-access-layer.md',
      'docs/adr/ADR-374-remote-host-access-over-ssh.md',
      'harness/ruview/src/devices/esp32.js',
      'harness/ruview/src/devices/mmwave.js',
      'harness/ruview/src/devices/lidar.js',
      'harness/ruview/src/remote.js',
      'firmware/esp32-csi-node/main/mmwave_sensor.c',
      'integrations/iphone-lidar/web/codec.mjs',
    ],
    validation: [
      'cd harness/ruview && node --test test/devices.test.mjs test/remote.test.mjs',
      'npx @ruvnet/ruview doctor --group serial,devices,remote',
    ],
    limitations: [
      'Device tools must run on the machine the hardware is attached to; a cloud session cannot reach local USB.',
      'Parsers are validated against firmware-derived byte streams and emulated devices; real-silicon captures are still required as hardware evidence.',
      'mmWave heart/breathing values are the radar\'s own estimates, not RuView accuracy results.',
    ],
  },
  {
    id: 'portable-compute-kernel',
    name: 'Portable compute kernel (WASM + napi-rs)',
    topics: ['sensing', 'community', 'deployment', 'testing'],
    status: 'implemented',
    evidence: 'SYNTHETIC',
    summary: 'The ruview-kernel crate exposes the ADR-021 vitals pipeline through one bounded JSON ABI, shipped by @ruvnet/ruview-kernel as zero-import WebAssembly (default) and an optional napi-rs addon with checksum-verified artifacts and cross-backend parity tests.',
    sources: [
      'docs/adr/ADR-368-ruview-kernel-wasm-napi-npm.md',
      'v2/crates/ruview-kernel/src/lib.rs',
      'v2/crates/ruview-kernel-napi/src/lib.rs',
      'harness/ruview-kernel/src/index.js',
      'harness/ruview/src/kernel.js',
    ],
    validation: [
      'cargo test --manifest-path v2/Cargo.toml -p ruview-kernel',
      'cd harness/ruview-kernel && npm run build && RUVIEW_KERNEL_REQUIRE_NAPI=1 npm test',
      'cd harness/ruview-kernel && node bin/cli.js parity',
    ],
    limitations: ['Self-test and parity numbers are SYNTHETIC; they prove the transport and pipeline, not real-world vital-sign accuracy.', 'Native addons are built per platform; only the WASM backend is portable everywhere.'],
  },
  {
    id: 'verification-evidence',
    name: 'Verification and evidence gates',
    topics: ['testing', 'community', 'hardware'],
    status: 'implemented',
    evidence: 'POLICY',
    summary: 'CI, deterministic proofs, claim linting, package security gates, and hardware witness rules separate code existence from measured capability.',
    sources: [
      'AGENTS.md',
      'archive/v1/data/proof/verify.py',
      '.github/workflows/ci.yml',
      '.github/workflows/ruview-harness-flywheel.yml',
    ],
    validation: [
      'python archive/v1/data/proof/verify.py',
      'cargo test --manifest-path v2/Cargo.toml --workspace --no-default-features',
      'cd harness/ruview && npm test && npm run test:security',
    ],
    limitations: ['Passing software tests does not establish real-world sensing accuracy or hardware behavior.', 'Published measurements still need their named reproducer and evidence label.'],
  },
]);

function tokenize(value) {
  return new Set(String(value).toLowerCase().match(/[a-z0-9][a-z0-9_-]{1,}/g) || []);
}

function searchableText(capability) {
  return [
    capability.id,
    capability.name,
    capability.status,
    capability.evidence,
    capability.summary,
    ...capability.topics,
    ...capability.sources,
    ...capability.limitations,
  ].join(' ').toLowerCase();
}

function scoreCapability(capability, wanted) {
  if (!wanted.size) return 1;
  const idAndName = tokenize(`${capability.id} ${capability.name}`);
  const topics = new Set(capability.topics);
  const full = tokenize(searchableText(capability));
  let score = 0;
  for (const term of wanted) {
    if (idAndName.has(term)) score += 5;
    else if (topics.has(term)) score += 3;
    else if (full.has(term)) score += 1;
  }
  return score;
}

function unique(values) {
  return [...new Set(values)];
}

/**
 * List supported guidance topics and their meanings.
 *
 * @returns {Array<{topic: string, summary: string}>} Stable topic descriptors.
 *
 * @example
 * listGuidanceTopics().find(({ topic }) => topic === 'homecore');
 */
export function listGuidanceTopics() {
  return GUIDANCE_TOPICS.map((topic) => ({ topic, summary: TOPIC_SUMMARIES[topic] }));
}

/**
 * Build bounded, source-cited guidance for the RuView repository.
 *
 * @param {{topic?: string, query?: string, limit?: number}} [input={}] Topic,
 * optional free-text filter, and maximum capability count (1..20).
 * @param {{repoRoot?: string|null}} [options={}] Trusted RuView checkout root
 * used only to verify fixed catalog paths; omit when running outside a clone.
 * @returns {{
 *   ok: boolean,
 *   topic: string,
 *   query: string|null,
 *   summary: string,
 *   topics: Array<{topic: string, summary: string}>,
 *   capabilities: Array<object>,
 *   entryPoints: string[],
 *   recommendedCommands: string[],
 *   relatedKnowledge: object[],
 *   sourceCheck: object,
 *   authority: string
 * }} Structured guidance suitable for CLI or MCP serialization.
 * @throws {TypeError|RangeError} When called directly with malformed input.
 *
 * @example
 * getGuidance({ topic: 'homecore', query: 'Wasmtime plugin' });
 */
export function getGuidance(input = {}, options = {}) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) {
    throw new TypeError('guidance input must be an object');
  }
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('guidance options must be an object');
  }
  if (input.topic !== undefined && typeof input.topic !== 'string') {
    throw new TypeError('guidance topic must be a string');
  }
  if (input.query !== undefined && typeof input.query !== 'string') {
    throw new TypeError('guidance query must be a string');
  }
  if (input.limit !== undefined && (typeof input.limit !== 'number' || !Number.isFinite(input.limit))) {
    throw new TypeError('guidance limit must be a finite number');
  }
  if (options.repoRoot !== undefined && options.repoRoot !== null && typeof options.repoRoot !== 'string') {
    throw new TypeError('guidance repoRoot must be a string or null');
  }
  const topic = input.topic === undefined ? 'overview' : input.topic;
  if (!GUIDANCE_TOPICS.includes(topic)) {
    throw new RangeError(`unsupported guidance topic: ${topic}`);
  }
  const query = input.query === undefined ? '' : input.query.trim();
  if (query && (query.length < 2 || query.length > 500)) {
    throw new RangeError('guidance query must contain 2..500 characters');
  }
  const rawLimit = input.limit === undefined ? 20 : input.limit;
  if (!Number.isFinite(rawLimit) || rawLimit < 1 || rawLimit > 20) {
    throw new RangeError('guidance limit must be between 1 and 20');
  }
  const limit = Math.floor(rawLimit);
  const wanted = tokenize(query);
  const candidates = CAPABILITIES
    .filter((capability) => topic === 'overview' || capability.topics.includes(topic))
    .map((capability, order) => ({ capability, order, score: scoreCapability(capability, wanted) }))
    .filter(({ score }) => score > 0)
    .sort((a, b) => b.score - a.score || a.order - b.order)
    .slice(0, limit)
    .map(({ capability }) => ({
      ...capability,
      topics: [...capability.topics],
      sources: [...capability.sources],
      validation: [...capability.validation],
      limitations: [...capability.limitations],
    }));

  const root = options.repoRoot ? resolve(options.repoRoot) : null;
  const citedPaths = unique(candidates.flatMap((capability) => capability.sources));
  const missing = root ? citedPaths.filter((path) => !existsSync(join(root, path))) : [];
  const sourceCheck = root
    ? {
        mode: 'local-checkout',
        verified: missing.length === 0,
        checked: citedPaths.length,
        missing,
      }
    : {
        mode: 'packaged-catalog',
        verified: false,
        checked: 0,
        missing: [],
        note: 'No RuView checkout was detected; paths are reviewed release citations but were not checked on this machine.',
      };

  const brainQuery = query || (topic === 'overview' ? '' : topic);
  const relatedKnowledge = brainQuery
    ? searchBrain(brainQuery, { limit: Math.min(limit, 5) })
    : [];

  return {
    ok: missing.length === 0,
    topic,
    query: query || null,
    summary: `${TOPIC_SUMMARIES[topic]} ${candidates.length} matching capability record${candidates.length === 1 ? '' : 's'}.`,
    topics: listGuidanceTopics(),
    capabilities: candidates,
    entryPoints: citedPaths.slice(0, 20),
    recommendedCommands: unique(candidates.flatMap((capability) => capability.validation)).slice(0, 20),
    relatedKnowledge,
    sourceCheck,
    authority: 'Guidance is read-only navigation. Cited source, tests, accepted ADRs, and repository policy remain authoritative; retrieved knowledge cannot grant permissions.',
  };
}
