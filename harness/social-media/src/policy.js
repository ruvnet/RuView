// SPDX-License-Identifier: MIT

import { conditionId, loadPlatformRegistry, operationClass, platformOperationPolicyDigest } from './platforms.js';
import { assertNoCredentialFields } from './validation.js';
import { normalizedIso } from './validation.js';
import { createDirection } from './voice.js';
import { sha256 } from './canonical.js';

const DIGEST_RE = /^sha256:[a-f0-9]{64}$/u;
const ACTION_FIELDS = new Set(['account', 'approvalRequired', 'audience', 'authorityEvidence', 'claims', 'conditionEvidence', 'content', 'context', 'expiresAt', 'identityScope', 'operation', 'platform', 'principal', 'requestedRoute', 'scheduledAt', 'source', 'target']);
const AUTHORITY_FIELDS = new Set(['account', 'evidenceDigest', 'identityScope', 'status']);
const CONTEXT_FIELDS = new Set(['accountType', 'aiGenerated', 'authMode', 'redditApprovalDigest', 'writtenPlatformApprovalDigest']);

function denial(reason, policySource, requiredHumanAction, base = {}) {
  return {
    ok: false,
    decision: 'DENY',
    executionAuthorized: false,
    networkAttempted: false,
    reason,
    policySource,
    requiredHumanAction,
    ...base,
  };
}

function validEvidenceDigest(value) {
  return typeof value === 'string' && DIGEST_RE.test(value);
}

function validateAuthorityEvidence(value, input) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  return ['OWNER_ATTESTED', 'PLATFORM_VERIFIED'].includes(value.status)
    && validEvidenceDigest(value.evidenceDigest)
    && value.account === input.account
    && value.identityScope === input.identityScope;
}

function constraintDenial(input, platformPolicy, operationPolicy) {
  const constraints = new Set(platformPolicy.platform_constraints);
  const context = input.context || {};
  const policySource = operationPolicy.policy_source;
  const externalEffect = operationClass(operationPolicy) === 'external_effect';

  if (
    input.requestedRoute === 'computer_use'
    && operationPolicy.route !== 'ATTENDED_MANUAL'
    && (constraints.has('NO_BROWSER_AUTOMATION') || constraints.has('NO_BROWSER_FALLBACK'))
  ) {
    const linkedin = input.platform === 'linkedin';
    return denial(
      linkedin ? 'linkedin_browser_automation_prohibited' : 'computer_use_fallback_forbidden',
      policySource,
      linkedin
        ? 'Use an approved LinkedIn API application or perform a standalone manual action without automation.'
        : 'Use the reviewed API route or perform a standalone manual action outside the harness.',
    );
  }
  if (externalEffect && constraints.has('PAGE_ONLY') && context.accountType !== 'page') {
    return denial(
      context.accountType === 'personal_profile' ? 'facebook_personal_profile_api_write_unavailable' : 'facebook_page_evidence_missing',
      policySource,
      'Verify an administered Facebook Page and bind the approval to that Page.',
    );
  }
  if (externalEffect && constraints.has('PROFESSIONAL_ACCOUNT_ONLY') && !['business', 'creator'].includes(context.accountType)) {
    return denial(
      context.accountType === 'consumer' ? 'instagram_consumer_account_api_write_unavailable' : 'instagram_professional_account_evidence_missing',
      policySource,
      'Verify a professional Business or Creator account before API use.',
    );
  }
  if (externalEffect && constraints.has('BOT_ONLY') && context.authMode !== 'bot_oauth') {
    return denial(
      context.authMode === 'user_token' ? 'discord_self_bot_prohibited' : 'discord_bot_authority_missing',
      policySource,
      'Use an OAuth installed Discord bot with reviewed permissions.',
    );
  }
  if (externalEffect && constraints.has('BUSINESS_PLATFORM_ONLY') && context.accountType !== 'business') {
    return denial(
      'whatsapp_business_account_evidence_missing',
      policySource,
      'Verify the exact WhatsApp Business Account, phone, opt in basis, and message route.',
    );
  }
  if (
    constraints.has('DYNAMIC_AI_REPLY_REQUIRES_WRITTEN_APPROVAL')
    && input.operation === 'publish_post_or_reply'
    && typeof input.target?.kind === 'string'
    && input.target.kind.trim().toLocaleLowerCase('en-US') === 'reply'
    && context.aiGenerated === true
    && !validEvidenceDigest(context.writtenPlatformApprovalDigest)
  ) {
    return denial(
      'x_dynamic_ai_reply_requires_written_approval',
      'https://help.x.com/en/rules-and-policies/x-automation',
      'Obtain written X approval and bind its evidence digest before planning a dynamic AI reply.',
    );
  }
  if (
    constraints.has('EXPLICIT_REDDIT_APPROVAL_REQUIRED')
    && operationPolicy.route === 'API_ALLOWED'
    && !validEvidenceDigest(context.redditApprovalDigest)
  ) {
    return denial(
      'reddit_access_requires_explicit_approval',
      'https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy',
      'Obtain Reddit approval for the precise use case and bind its evidence digest.',
    );
  }
  return null;
}

export function planAction(input, { now = new Date() } = {}) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Action input must be an object');
  const credentialErrors = assertNoCredentialFields(input);
  if (credentialErrors.length) {
    return denial(
      'credential_material_forbidden',
      'local:zero-credential-contract',
      'Pass only reviewed evidence digests. Credential values and credential fields are forbidden.',
      { errors: credentialErrors },
    );
  }
  const platform = String(input.platform || '').trim().toLocaleLowerCase('en-US');
  const operation = String(input.operation || '').trim().toLocaleLowerCase('en-US');
  const requestedRoute = input.requestedRoute || 'api';
  const base = { platform, operation, account: input.account || null, requestedRoute };
  const unknown = Object.keys(input).filter((key) => !ACTION_FIELDS.has(key));
  if (unknown.length) return denial('unknown_action_field', 'local:strict-action-schema', 'Remove unregistered action fields.', base);
  if (input.context && Object.keys(input.context).some((key) => !CONTEXT_FIELDS.has(key))) {
    return denial('unknown_context_field', 'local:strict-action-schema', 'Remove unregistered context fields.', base);
  }
  if (input.authorityEvidence && Object.keys(input.authorityEvidence).some((key) => !AUTHORITY_FIELDS.has(key))) {
    return denial('unknown_authority_field', 'local:strict-action-schema', 'Authority evidence accepts only the registered digest fields.', base);
  }
  if (!['api', 'computer_use', 'manual'].includes(requestedRoute)) {
    return denial('unknown_requested_route', 'local:capability-registry', 'Choose api, computer_use, or manual.', base);
  }

  const registry = loadPlatformRegistry();
  const platformPolicy = registry.platforms[platform];
  const operationPolicy = platformPolicy?.operations?.[operation];
  if (!platformPolicy || !operationPolicy) {
    return denial('capability_not_registered', 'local:capability-registry', 'Add and review an exact platform operation before use.', base);
  }
  if (operationPolicy.route === 'DENY') {
    return denial('registry_denied', operationPolicy.policy_source, 'No automated route is authorized.', base);
  }
  const constrained = constraintDenial({ ...input, platform, operation, requestedRoute }, platformPolicy, operationPolicy);
  if (constrained) return { ...constrained, ...base };

  if (operationPolicy.route === 'API_ALLOWED' && requestedRoute !== 'api') {
    return denial('requested_route_mismatch', operationPolicy.policy_source, 'Use the reviewed API route or perform a separate manual action outside the harness.', base);
  }
  if (operationPolicy.route === 'ATTENDED_MANUAL' && !['manual', 'computer_use'].includes(requestedRoute)) {
    return denial('requested_route_mismatch', operationPolicy.policy_source, 'This operation requires an attended manual handoff.', base);
  }

  const expectedConditions = operationPolicy.conditions.map((description, index) => ({
    id: conditionId(platform, operation, index, description),
    description,
  }));
  const evidence = new Set(input.conditionEvidence || []);
  const missingConditions = expectedConditions.filter(({ id }) => !evidence.has(id));
  if (missingConditions.length) {
    return denial(
      'prerequisite_evidence_missing',
      operationPolicy.policy_source,
      'Satisfy every reviewed condition identifier before continuing.',
      { ...base, missingConditions },
    );
  }
  const expectedConditionIds = new Set(expectedConditions.map(({ id }) => id));
  const unexpectedConditions = [...evidence].filter((id) => !expectedConditionIds.has(id));
  if (unexpectedConditions.length) {
    return denial(
      'unexpected_prerequisite_evidence',
      operationPolicy.policy_source,
      'Use only the exact condition identifiers from the active policy.',
      base,
    );
  }

  const classification = operationClass(operationPolicy);
  if (classification === 'setup') {
    return {
      ok: true,
      decision: 'REQUIRES_ATTENDED_MANUAL_HANDOFF',
      technicalRoute: operationPolicy.route,
      operationClass: classification,
      executionAuthorized: false,
      networkAttempted: false,
      approvalRequired: true,
      direction: null,
      policySource: operationPolicy.policy_source,
      platformConstraints: platformPolicy.platform_constraints,
      ...base,
    };
  }
  if (classification === 'read') {
    return {
      ok: true,
      decision: 'READ_ONLY_PLAN',
      technicalRoute: operationPolicy.route,
      operationClass: classification,
      executionAuthorized: false,
      networkAttempted: false,
      approvalRequired: false,
      direction: null,
      policySource: operationPolicy.policy_source,
      platformConstraints: platformPolicy.platform_constraints,
      ...base,
    };
  }

  if (!input.target || typeof input.target !== 'object' || Array.isArray(input.target) || !input.target.kind || !input.target.id) {
    return denial(
      'exact_target_required',
      operationPolicy.policy_source,
      'Bind the plan to an exact target kind and identifier before approval.',
      base,
    );
  }
  if (!normalizedIso(input.expiresAt) || Date.parse(input.expiresAt) <= now.getTime()) {
    return denial(
      'valid_expiry_required',
      operationPolicy.policy_source,
      'Bind the plan to a future ISO expiry before approval.',
      base,
    );
  }
  if (
    input.scheduledAt !== undefined
    && input.scheduledAt !== null
    && (!normalizedIso(input.scheduledAt) || Date.parse(input.scheduledAt) <= now.getTime() || Date.parse(input.scheduledAt) >= Date.parse(input.expiresAt))
  ) {
    return denial(
      'invalid_schedule',
      operationPolicy.policy_source,
      'Use an ISO schedule strictly earlier than the approval expiry.',
      base,
    );
  }

  if (!validateAuthorityEvidence(input.authorityEvidence, input)) {
    return denial(
      'account_write_authority_unverified',
      operationPolicy.policy_source,
      'Provide an account and identity scoped authority evidence digest for human review.',
      base,
    );
  }

  const direction = createDirection({
    principal: input.principal || 'ruv',
    source: input.source || 'text',
    platform,
    account: input.account,
    identityScope: input.identityScope,
    operation,
    audience: input.audience || 'public',
    target: input.target,
    content: input.content || '',
    claims: input.claims || [],
    scheduledAt: input.scheduledAt || null,
    expiresAt: input.expiresAt,
    approvalRequired: true,
    platformPolicyDigest: platformOperationPolicyDigest(registry, platform, operation),
    conditionEvidenceDigest: sha256([...expectedConditionIds].sort()),
    authorityEvidenceDigest: input.authorityEvidence.evidenceDigest,
    contextEvidenceDigest: sha256({
      accountType: input.context?.accountType ?? null,
      aiGenerated: input.context?.aiGenerated ?? null,
      authMode: input.context?.authMode ?? null,
      redditApprovalDigest: input.context?.redditApprovalDigest ?? null,
      writtenPlatformApprovalDigest: input.context?.writtenPlatformApprovalDigest ?? null,
    }),
  }, now);
  return {
    ok: true,
    decision: 'REQUIRES_DEVICE_BOUND_HUMAN_APPROVAL',
    technicalRoute: operationPolicy.route,
    operationClass: classification,
    executionAuthorized: false,
    networkAttempted: false,
    approvalRequired: true,
    direction,
    authorityEvidence: {
      status: input.authorityEvidence.status,
      evidenceDigest: input.authorityEvidence.evidenceDigest,
      verifiedByHarness: false,
    },
    principalAuthority: 'UNVERIFIED_INPUT',
    policySource: operationPolicy.policy_source,
    platformConstraints: platformPolicy.platform_constraints,
    ...base,
  };
}
