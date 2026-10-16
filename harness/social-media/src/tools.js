// SPDX-License-Identifier: MIT

import { runAutopilot } from './autopilot.js';
import { verifyReceiptChain } from './audit.js';
import { runDoctor } from './doctor.js';
import { evaluateCandidate } from './flywheel.js';
import { normalizeSnapshot, SUPPORTED_OPTIMIZATION_METRICS } from './metrics.js';
import { listPlatformCapabilities } from './platforms.js';
import { planAction } from './policy.js';
import { assertNoCredentialFields, validateArguments } from './validation.js';
import { getDirectionPolicy, lintContent } from './voice.js';
import { redactDeep, redactString } from './sensitive.js';
import { loadResearchBaseline } from './research.js';

const EMPTY_SCHEMA = Object.freeze({ type: 'object', properties: {}, additionalProperties: false });
const CLAIM_SCHEMA = Object.freeze({
  type: 'object',
  required: ['grade', 'source_url', 'measured_at'],
  properties: {
    grade: { type: 'string', enum: ['MEASURED', 'CLAIMED', 'SYNTHETIC'] },
    source_url: { type: 'string', minLength: 8, maxLength: 2048 },
    measured_at: { type: 'string', minLength: 20, maxLength: 64 },
    reproducer: { type: 'string', maxLength: 2048 },
  },
  additionalProperties: false,
});
const DIGEST_SCHEMA = Object.freeze({ type: 'string', pattern: '^sha256:[a-f0-9]{64}$' });
const EXPERIMENT_PLAN_SCHEMA = Object.freeze({
  type: 'object',
  required: ['schema', 'platform', 'identityScope', 'objective', 'metric', 'metricSemanticsDigest', 'direction', 'minimumSamples', 'minimumRelativeLift', 'pairingRuleDigest', 'anchorSetDigest', 'policyDigest', 'registeredAt', 'observationStartsAt'],
  properties: {
    schema: { type: 'string', enum: ['ExperimentPlanV1'] },
    platform: { type: 'string', minLength: 1, maxLength: 32 },
    identityScope: { type: 'string', enum: ['ruv_personal', 'ruvnet', 'agentics', 'cognitum'] },
    objective: { type: 'string', minLength: 1, maxLength: 128 },
    metric: { type: 'string', enum: SUPPORTED_OPTIMIZATION_METRICS },
    metricSemanticsDigest: DIGEST_SCHEMA,
    direction: { type: 'string', enum: ['increase', 'decrease'] },
    minimumSamples: { type: 'integer', minimum: 20, maximum: 10000 },
    minimumRelativeLift: { type: 'number', minimum: 0.05, maximum: 10 },
    pairingRuleDigest: DIGEST_SCHEMA,
    anchorSetDigest: DIGEST_SCHEMA,
    policyDigest: DIGEST_SCHEMA,
    registeredAt: { type: 'string', minLength: 24, maxLength: 24 },
    observationStartsAt: { type: 'string', minLength: 24, maxLength: 24 },
  },
  additionalProperties: false,
});

function gateReceiptSchema(blockedActions = false) {
  return {
    type: 'object',
    required: ['schema', 'gate', 'datasetDigest', 'experimentPlanDigest', 'outcome', 'evidenceDigests', 'issuerEvidenceDigest', 'issuedAt', 'expiresAt', 'receiptDigest', ...(blockedActions ? ['blockedActionCount'] : [])],
    properties: {
      schema: { type: 'string', enum: ['SocialEvaluationGateV1'] },
      gate: { type: 'string', enum: [blockedActions ? 'blockedActions' : 'anchor', ...(blockedActions ? [] : ['provenance', 'security'])] },
      datasetDigest: DIGEST_SCHEMA,
      experimentPlanDigest: DIGEST_SCHEMA,
      outcome: { type: 'string', enum: ['PASS', 'FAIL'] },
      evidenceDigests: { type: 'array', minItems: 1, maxItems: 32, items: DIGEST_SCHEMA },
      issuerEvidenceDigest: DIGEST_SCHEMA,
      issuedAt: { type: 'string', minLength: 24, maxLength: 24 },
      expiresAt: { type: 'string', minLength: 24, maxLength: 24 },
      receiptDigest: DIGEST_SCHEMA,
      ...(blockedActions ? { blockedActionCount: { type: 'integer', minimum: 0 } } : {}),
    },
    additionalProperties: false,
  };
}

const EVALUATION_INPUT_SCHEMA = Object.freeze({
  type: 'object',
  required: ['experimentPlan', 'experimentPlanDigest', 'datasetDigest', 'baseline', 'variant', 'observationBindings', 'gateReceipts'],
  properties: {
    experimentPlan: EXPERIMENT_PLAN_SCHEMA,
    experimentPlanDigest: DIGEST_SCHEMA,
    datasetDigest: DIGEST_SCHEMA,
    baseline: { type: 'array', maxItems: 10000, items: { type: 'number' } },
    variant: { type: 'array', maxItems: 10000, items: { type: 'number' } },
    observationBindings: {
      type: 'array',
      minItems: 1,
      maxItems: 10000,
      items: {
        type: 'object',
        required: ['schema', 'pairingKeyDigest', 'baselineSnapshotDigest', 'variantSnapshotDigest', 'baselineMetricSemanticsDigest', 'variantMetricSemanticsDigest'],
        properties: {
          schema: { type: 'string', enum: ['MetricObservationPairV1'] },
          pairingKeyDigest: DIGEST_SCHEMA,
          baselineSnapshotDigest: DIGEST_SCHEMA,
          variantSnapshotDigest: DIGEST_SCHEMA,
          baselineMetricSemanticsDigest: DIGEST_SCHEMA,
          variantMetricSemanticsDigest: DIGEST_SCHEMA,
        },
        additionalProperties: false,
      },
    },
    gateReceipts: {
      type: 'object',
      required: ['anchor', 'provenance', 'security', 'blockedActions'],
      properties: {
        anchor: gateReceiptSchema(false),
        provenance: gateReceiptSchema(false),
        security: gateReceiptSchema(false),
        blockedActions: gateReceiptSchema(true),
      },
      additionalProperties: false,
    },
  },
  additionalProperties: false,
});

const OPTIMIZATION_PROPOSAL_SCHEMA = Object.freeze({
  type: 'object',
  required: ['schema', 'proposalId', 'platform', 'account', 'identityScope', 'changeType', 'oneChange', 'rationale', 'expectedEffect', 'rollback', 'sourceDigests', 'experimentPlanDigest', 'datasetDigest', 'createdAt', 'expiresAt', 'proposalDigest'],
  properties: {
    schema: { type: 'string', enum: ['OptimizationProposalV1'] },
    proposalId: { type: 'string', minLength: 1, maxLength: 64 },
    platform: { type: 'string', minLength: 1, maxLength: 32 },
    account: { type: 'string', minLength: 1, maxLength: 128 },
    identityScope: { type: 'string', enum: ['ruv_personal', 'ruvnet', 'agentics', 'cognitum'] },
    changeType: { type: 'string', enum: ['CONTENT_STRUCTURE', 'EVIDENCE_PRESENTATION', 'TIMING_HYPOTHESIS', 'VOICE_RULE'] },
    oneChange: { type: 'string', minLength: 1, maxLength: 2000 },
    rationale: { type: 'string', minLength: 1, maxLength: 4000 },
    expectedEffect: { type: 'string', minLength: 1, maxLength: 1024 },
    rollback: { type: 'string', minLength: 1, maxLength: 2048 },
    sourceDigests: { type: 'array', minItems: 2, maxItems: 16, items: DIGEST_SCHEMA },
    experimentPlanDigest: DIGEST_SCHEMA,
    datasetDigest: DIGEST_SCHEMA,
    createdAt: { type: 'string', minLength: 24, maxLength: 24 },
    expiresAt: { type: 'string', minLength: 24, maxLength: 24 },
    proposalDigest: DIGEST_SCHEMA,
  },
  additionalProperties: false,
});

const AUTOPILOT_CHECKPOINT_SCHEMA = Object.freeze({
  type: 'object',
  required: ['schema', 'runId', 'batchDigest', 'identityRegistryDigest', 'scopeDigest', 'nextCursor', 'processedProposalDigests', 'previousCheckpointDigest', 'checkpointDigest'],
  properties: {
    schema: { type: 'string', enum: ['AutopilotCheckpointV1'] },
    runId: { type: 'string', minLength: 1, maxLength: 64, pattern: '^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$' },
    batchDigest: DIGEST_SCHEMA,
    identityRegistryDigest: DIGEST_SCHEMA,
    scopeDigest: DIGEST_SCHEMA,
    nextCursor: { type: 'integer', minimum: 0, maximum: 100 },
    processedProposalDigests: { type: 'array', maxItems: 100, items: DIGEST_SCHEMA },
    previousCheckpointDigest: { type: ['string', 'null'], pattern: '^sha256:[a-f0-9]{64}$' },
    checkpointDigest: DIGEST_SCHEMA,
  },
  additionalProperties: false,
});

const TOOLS = Object.freeze({
  social_doctor: {
    description: 'Validate static policy, research, secret scan, zero dependency, and runtime invariants without network access.',
    inputSchema: EMPTY_SCHEMA,
    run: () => runDoctor(),
  },
  social_platforms: {
    description: 'List reviewed platform capabilities and their API, attended manual, or deny route.',
    inputSchema: EMPTY_SCHEMA,
    run: () => ({ ok: true, schema: 'SocialCapabilitiesV1', capabilities: listPlatformCapabilities() }),
  },
  social_research_baseline: {
    description: 'Return the dated public and unauthenticated RuVnet social identity baseline.',
    inputSchema: EMPTY_SCHEMA,
    run: () => ({
      ok: true,
      networkAttempted: false,
      baseline: loadResearchBaseline(),
    }),
  },
  social_direction_policy: {
    description: 'Return the static rUv voice, identity separation, and evidence policy.',
    inputSchema: EMPTY_SCHEMA,
    run: () => ({ ok: true, policy: getDirectionPolicy() }),
  },
  social_direction_check: {
    description: 'Lint draft content against a channel voice and quantitative evidence rules. It never publishes.',
    inputSchema: {
      type: 'object',
      required: ['platform', 'text'],
      properties: {
        platform: { type: 'string', minLength: 1, maxLength: 32 },
        text: { type: 'string', minLength: 1, maxLength: 10000 },
        claims: { type: 'array', maxItems: 64, items: CLAIM_SCHEMA },
      },
      additionalProperties: false,
    },
    run: (args) => lintContent(args),
  },
  social_action_plan: {
    description: 'Evaluate a proposed action against the capability registry and create a non-executable approval plan. No network request is made.',
    inputSchema: {
      type: 'object',
      required: ['platform', 'operation', 'requestedRoute'],
      properties: {
        platform: { type: 'string', minLength: 1, maxLength: 32 },
        operation: { type: 'string', minLength: 1, maxLength: 96 },
        requestedRoute: { type: 'string', enum: ['api', 'computer_use', 'manual'] },
        principal: { type: 'string', minLength: 1, maxLength: 128 },
        account: { type: 'string', minLength: 1, maxLength: 128 },
        identityScope: { type: 'string', minLength: 1, maxLength: 64 },
        source: { type: 'string', enum: ['text', 'voice'] },
        audience: { type: 'string', minLength: 1, maxLength: 64 },
        target: {
          type: 'object',
          required: ['kind', 'id'],
          properties: {
            kind: { type: 'string', minLength: 1, maxLength: 64 },
            id: { type: 'string', minLength: 1, maxLength: 512 },
            parentId: { type: 'string', minLength: 1, maxLength: 512 }
          },
          additionalProperties: false
        },
        content: { type: 'string', maxLength: 10000 },
        claims: { type: 'array', maxItems: 64, items: CLAIM_SCHEMA },
        scheduledAt: { type: 'string', maxLength: 64 },
        expiresAt: { type: 'string', maxLength: 64 },
        conditionEvidence: {
          type: 'array',
          maxItems: 64,
          items: { type: 'string', minLength: 26, maxLength: 26, pattern: '^condition:[a-f0-9]{16}$' }
        },
        authorityEvidence: {
          type: 'object',
          required: ['status', 'evidenceDigest', 'account', 'identityScope'],
          properties: {
            status: { type: 'string', enum: ['OWNER_ATTESTED', 'PLATFORM_VERIFIED'] },
            evidenceDigest: { type: 'string', pattern: '^sha256:[a-f0-9]{64}$' },
            account: { type: 'string', minLength: 1, maxLength: 128 },
            identityScope: { type: 'string', minLength: 1, maxLength: 64 }
          },
          additionalProperties: false
        },
        context: {
          type: 'object',
          properties: {
            accountType: { type: 'string', maxLength: 64 },
            aiGenerated: { type: 'boolean' },
            authMode: { type: 'string', maxLength: 64 },
            redditApprovalDigest: { type: 'string', pattern: '^sha256:[a-f0-9]{64}$' },
            writtenPlatformApprovalDigest: { type: 'string', pattern: '^sha256:[a-f0-9]{64}$' }
          },
          additionalProperties: false
        }
      },
      additionalProperties: false
    },
    run: (args) => planAction(args),
  },
  social_metrics_normalize: {
    description: 'Normalize one platform metric snapshot with explicit denominators and prohibit cross-platform ranking.',
    inputSchema: {
      type: 'object',
      required: ['platform', 'account', 'identityScope', 'collectionMode', 'connectorDefinitionVersion', 'contentId', 'contentDigest', 'sourceDigest', 'provenanceDigest', 'evidenceLabel', 'windowStart', 'windowEnd', 'collectedAt', 'qualityFlags', 'counters', 'definitions', 'rateDefinitions'],
      properties: {
        platform: { type: 'string', minLength: 1, maxLength: 32 },
        account: { type: 'string', minLength: 1, maxLength: 128 },
        identityScope: { type: 'string', enum: ['ruv_personal', 'ruvnet', 'agentics', 'cognitum'] },
        collectionMode: { type: 'string', enum: ['PLATFORM_EXPORT', 'PUBLIC_PAGE', 'SYNTHETIC_FIXTURE'] },
        connectorDefinitionVersion: { type: 'string', minLength: 1, maxLength: 128 },
        contentId: { type: 'string', minLength: 1, maxLength: 512 },
        contentDigest: DIGEST_SCHEMA,
        sourceDigest: DIGEST_SCHEMA,
        provenanceDigest: { type: 'string', pattern: '^sha256:[a-f0-9]{64}$' },
        evidenceLabel: { type: 'string', enum: ['MEASURED', 'CLAIMED', 'SYNTHETIC'] },
        windowStart: { type: 'string', minLength: 20, maxLength: 64 },
        windowEnd: { type: 'string', minLength: 20, maxLength: 64 },
        collectedAt: { type: 'string', minLength: 20, maxLength: 64 },
        qualityFlags: { type: 'array', minItems: 1, maxItems: 6, items: { type: 'string', enum: ['DELAYED', 'ESTIMATED', 'FILTERED', 'NONE', 'ROUNDED', 'SAMPLED'] } },
        counters: { type: 'object', properties: {}, additionalProperties: true },
        definitions: { type: 'object', properties: {}, additionalProperties: true },
        rateDefinitions: {
          type: 'array',
          minItems: 1,
          maxItems: 32,
          items: {
            type: 'object',
            required: ['name', 'numerators', 'denominator'],
            properties: {
              name: { type: 'string', minLength: 1, maxLength: 64 },
              numerators: { type: 'array', minItems: 1, maxItems: 16, items: { type: 'string', minLength: 1, maxLength: 64 } },
              denominator: { type: 'string', minLength: 1, maxLength: 64 }
            },
            additionalProperties: false
          }
        }
      },
      additionalProperties: false,
    },
    run: (args) => ({ ok: true, normalized: normalizeSnapshot(args) }),
  },
  social_flywheel_evaluate: {
    description: 'Screen paired observations against a frozen plan and integrity-bound gate evidence. Gate authority remains unverified and no policy can be promoted.',
    inputSchema: EVALUATION_INPUT_SCHEMA,
    run: (args) => ({ ok: true, evaluation: evaluateCandidate(args) }),
  },
  social_autopilot_run: {
    description: 'Run a bounded and restartable proposal evaluation loop. It can only reject candidates or queue digests for independent verification and cannot execute, promote, or mutate policy.',
    inputSchema: {
      type: 'object',
      required: ['runId', 'maximumCycles', 'proposals'],
      properties: {
        runId: { type: 'string', minLength: 1, maxLength: 64, pattern: '^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$' },
        maximumCycles: { type: 'integer', minimum: 1, maximum: 100 },
        proposals: {
          type: 'array',
          minItems: 1,
          maxItems: 100,
          items: {
            type: 'object',
            required: ['proposal', 'evaluation'],
            properties: {
              proposal: OPTIMIZATION_PROPOSAL_SCHEMA,
              evaluation: EVALUATION_INPUT_SCHEMA,
            },
            additionalProperties: false,
          },
        },
        checkpoint: AUTOPILOT_CHECKPOINT_SCHEMA,
      },
      additionalProperties: false,
    },
    run: (args) => ({ ok: true, run: runAutopilot(args) }),
  },
  social_audit_verify: {
    description: 'Verify an in-memory digest-only social audit chain against an externally retained expected head and count.',
    inputSchema: {
      type: 'object',
      required: ['receipts', 'expectedHead', 'expectedCount'],
      properties: {
        receipts: { type: 'array', maxItems: 10000, items: { type: 'object', properties: {}, additionalProperties: true } },
        expectedHead: { type: ['string', 'null'] },
        expectedCount: { type: 'integer', minimum: 0, maximum: 10000 },
      },
      additionalProperties: false,
    },
    run: ({ receipts, expectedHead, expectedCount }) => verifyReceiptChain(receipts, { expectedHead, expectedCount }),
  },
});

export function listTools() {
  return Object.entries(TOOLS).map(([name, tool]) => ({
    name,
    description: tool.description,
    inputSchema: tool.inputSchema,
    annotations: {
      readOnlyHint: true,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: false,
    },
  }));
}

export async function runTool(name, args = {}) {
  const tool = TOOLS[name];
  if (!tool) return { ok: false, error: 'unknown_tool' };
  const credentialErrors = assertNoCredentialFields(args);
  if (credentialErrors.length) return { ok: false, error: 'credential_material_forbidden', details: credentialErrors };
  const errors = validateArguments(tool.inputSchema, args);
  if (errors.length) return { ok: false, error: 'invalid_arguments', details: errors };
  try {
    return redactDeep(await tool.run(args));
  } catch (cause) {
    return { ok: false, error: 'tool_failed', message: redactString(cause instanceof Error ? cause.message : String(cause)) };
  }
}
