import assert from 'node:assert/strict';
import test from 'node:test';

import { controlPlaneResponse } from '../src/http-server.js';

test('health endpoint exposes bounded read only status', () => {
  const response = controlPlaneResponse('GET', '/healthz');
  assert.equal(response.status, 200);
  assert.equal(response.body.ok, true);
  assert.equal(Object.hasOwn(response.body, 'environment'), false);
});

test('capability endpoint cannot authorize execution', () => {
  const response = controlPlaneResponse('GET', '/v1/capabilities');
  assert.equal(response.body.authority, 'read_only_zero_credential');
  assert.equal(response.body.executionAuthorized, false);
  assert.equal(response.body.capabilities.length, 10);
});

test('write methods are rejected', () => {
  const response = controlPlaneResponse('POST', '/v1/capabilities');
  assert.equal(response.status, 405);
});
