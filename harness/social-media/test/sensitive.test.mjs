import assert from 'node:assert/strict';
import test from 'node:test';

import { REDACTED, redactDeep, scanSensitive } from '../src/sensitive.js';

test('credential shaped values and capability URLs are detected', () => {
  const githubValue = `ghp_${'q'.repeat(32)}`;
  const githubFineGrainedValue = `github_pat_${'q'.repeat(64)}`;
  const discordWebhook = `https://discord.com/api/webhooks/123456789/${'w'.repeat(48)}`;
  const inviteValue = `https://discord.${'gg'}/${'a'.repeat(12)}`;
  assert.equal(scanSensitive({ account: githubValue })[0].rule, 'github_token');
  assert.equal(scanSensitive({ account: githubFineGrainedValue })[0].rule, 'github_fine_grained_token');
  assert.equal(scanSensitive({ account: discordWebhook })[0].rule, 'discord_webhook');
  assert.equal(scanSensitive({ link: inviteValue })[0].rule, 'discord_invite');
});

test('defensive output redaction covers keys and values', () => {
  const sensitiveField = `access_${'token'}`;
  const githubValue = `ghp_${'q'.repeat(32)}`;
  const output = redactDeep({
    [sensitiveField]: 'opaque',
    account: githubValue,
    safe: 'public',
  });
  assert.equal(output[sensitiveField], REDACTED);
  assert.equal(output.account, REDACTED);
  assert.equal(output.safe, 'public');
  assert.doesNotMatch(JSON.stringify(output), new RegExp(githubValue, 'u'));
});
