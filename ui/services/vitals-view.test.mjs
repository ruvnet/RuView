// Regression tests for the vitals presentation rules.
//
// Requested by the reviewer of the live-view PR ("add deterministic regressions"
// for the two changed-code defects). The first of the two was: "stale numeric
// vitals remain displayed after the governed frame abstains/room becomes empty".
//
// Run: node --test ui/services/vitals-view.test.mjs
// (also listed in .github/workflows/ci.yml so CI executes it)

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { vitalsView, VITALS_CURRENT_GRACE_S, VITALS_HISTORY_S } from './vitals-view.js';

const published = { breathing_rate_bpm: 12.2, heart_rate_bpm: 84 };

test('a young publication on an authoritative frame is shown as current', () => {
  const v = vitalsView({
    vitals: published, ageSeconds: 0, authorityOpen: true, reason: null,
  });
  assert.equal(v.state, 'current');
  assert.equal(v.heart, '84 bpm');
  assert.equal(v.breathing, '12.2 rpm');
  assert.equal(v.historical, null, 'a current value is not also labelled historical');
});

test('authority closing clears the numeric fields and downgrades the last value', () => {
  // The exact rejected behaviour: authority closes while the panel keeps showing
  // the number as if it were live.
  const v = vitalsView({
    vitals: published, ageSeconds: VITALS_CURRENT_GRACE_S + 4,
    authorityOpen: false,
    reason: 'fresh explicit calibration with exactly one occupant and qualified evidence required',
  });
  assert.equal(v.state, 'withheld');
  assert.equal(v.heart, 'withheld', 'no number may be presented as current');
  assert.equal(v.breathing, 'withheld');
  assert.ok(v.historical, 'the last value is still reported, explicitly as history');
  assert.match(v.historical.text, /84 bpm/);
  assert.match(v.historical.text, /ago/);
  assert.equal(v.historical.ageSeconds, VITALS_CURRENT_GRACE_S + 4);
  assert.ok(!/^[\d.]/.test(v.heart), 'the current field must not start with a number');
});

test('a closed frame withholds even when the publication is one second old', () => {
  // The gate can close on the very next frame; age alone must not keep a value
  // current, because the authorization is per governed frame.
  const v = vitalsView({
    vitals: published, ageSeconds: 1, authorityOpen: false, reason: null,
  });
  assert.equal(v.state, 'withheld');
  assert.equal(v.heart, 'withheld');
});

test('an open authority with an old publication is historical, not current', () => {
  // Socket open but silent: the last message said "open", yet the value is old.
  const v = vitalsView({
    vitals: published, ageSeconds: VITALS_CURRENT_GRACE_S + 1,
    authorityOpen: true, reason: null,
  });
  assert.equal(v.state, 'withheld');
  assert.equal(v.heart, 'withheld');
  assert.ok(v.historical);
});

test('nothing ever published reads as abstained with the server reason', () => {
  const v = vitalsView({
    vitals: null, ageSeconds: null, authorityOpen: false,
    reason: 'needs one occupant',
  });
  assert.equal(v.state, 'none');
  assert.equal(v.heart, 'abstained');
  assert.equal(v.breathing, 'abstained');
  assert.equal(v.note, 'needs one occupant');
  assert.equal(v.historical, null);
});

test('a publication older than the history window is dropped entirely', () => {
  const v = vitalsView({
    vitals: published, ageSeconds: VITALS_HISTORY_S + 1,
    authorityOpen: false, reason: null,
  });
  assert.equal(v.state, 'withheld');
  assert.equal(v.heart, 'withheld');
  assert.equal(v.historical, null, 'an ancient value must not linger as history');
});

test('a partial publication only reports the metric that exists', () => {
  const v = vitalsView({
    vitals: { breathing_rate_bpm: null, heart_rate_bpm: 63 },
    ageSeconds: 0, authorityOpen: true, reason: null,
  });
  assert.equal(v.state, 'current');
  assert.equal(v.heart, '63 bpm');
  assert.equal(v.breathing, 'abstained');
});

test('the grace boundary is inclusive', () => {
  const atGrace = vitalsView({
    vitals: published, ageSeconds: VITALS_CURRENT_GRACE_S,
    authorityOpen: true, reason: null,
  });
  assert.equal(atGrace.state, 'current');
  const pastGrace = vitalsView({
    vitals: published, ageSeconds: VITALS_CURRENT_GRACE_S + 0.5,
    authorityOpen: true, reason: null,
  });
  assert.equal(pastGrace.state, 'withheld');
});
