// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseSpacesOutput, listCognitumSpaces, SPATIAL_RESOURCE_KINDS } from '../src/spaces.js';

const legacy = { object: 'list', data: [{ id: 'room-1', tenantId: 'tenant-1', siteId: 'site-1', name: 'Room', privacy: 'P2', state: { classification: 'P2', confidence: 0.9 } }], boundary: { authoritativeState: 'HomeCore Edge', excluded: ['raw_csi', 'cir', 'rf_tensors', 'recordings', 'pose_frames', 'vital_waveforms', 'identity_observations'] } };

test('a legacy installed CLI cannot label room data as events or alerts', { skip: process.platform === 'win32' }, async () => {
  const root = mkdtempSync(join(tmpdir(), 'ruview-legacy-kind-'));
  const binary = join(root, 'legacy-cli');
  writeFileSync(binary, `#!${process.execPath}\nprocess.stdout.write(${JSON.stringify(JSON.stringify(legacy))});\n`, { mode: 0o755 });
  try {
    for (const resource of ['events', 'alerts']) {
      const result = await listCognitumSpaces({ resource }, { binary, env: {} });
      assert.equal(result.ok, false, `${resource} must not return legacy room data`);
      assert.equal(result.reason, 'invalid_spaces_output');
    }
    const result = await listCognitumSpaces({ resource: 'spaces' }, { binary, env: {} });
    assert.equal(result.ok, true);
    assert.equal(result.resource, 'spaces');
    assert.deepEqual(result.data, legacy.data);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('legacy space envelopes are accepted only for the legacy collection', () => {
  const raw = JSON.stringify(legacy);
  assert.deepEqual(parseSpacesOutput(raw), legacy);
  assert.deepEqual(parseSpacesOutput(raw, 'spaces'), legacy);
  for (const kind of SPATIAL_RESOURCE_KINDS.filter(kind => kind !== 'spaces')) {
    assert.throws(() => parseSpacesOutput(raw, kind), /legacy.*spaces/i);
    assert.throws(() => parseSpacesOutput(JSON.stringify({ ...legacy, data: [] }), kind), /legacy.*spaces/i);
  }
});

test('all eight versioned collections still validate their own contracts', () => {
  for (const kind of SPATIAL_RESOURCE_KINDS) {
    const item = { id: 'item-1', tenantId: 'tenant-1', workspaceId: '11111111-1111-7111-8111-111111111111', siteId: 'site-1', buildingId: 'building-1', floorId: 'floor-1', spaceId: 'room-1', kind, schemaVersion: '1.0', messageId: 'message-1', eventSequence: 1, version: 1, privacy: 'P2', confidence: 0.9, provenance: {}, attributes: {}, observedAt: '2026-08-19T00:00:00Z', expiresAt: null, entityType: 'sensor', eventType: 'occupancy.changed', alertType: 'occupancy', severity: 'warning', status: 'open' };
    const response = { ...legacy, schemaVersion: '1.0', kind, data: [item], nextCursor: null };
    assert.deepEqual(parseSpacesOutput(JSON.stringify(response), kind), response);
    assert.throws(() => parseSpacesOutput(JSON.stringify(response), kind === 'spaces' ? 'events' : 'spaces'), /contract version or kind/i);
  }
});
