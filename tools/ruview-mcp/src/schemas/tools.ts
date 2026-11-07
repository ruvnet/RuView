/**
 * Zod input schemas for all 20 ADR-124 MCP tools.
 *
 * §4.1  — 15 sensing tools (presence, vitals, pose, primitives, bfld, node, vector)
 * §4.1a — 5 policy / governance tools (RUVIEW-POLICY)
 *
 * Each exported schema is named `<CamelCase>InputSchema` matching the tool
 * name from the ADR-124 §4.1 catalog table. The parallel `TOOL_NAMES` array
 * is the single source of truth asserted by the schema-coverage test.
 */

import { z } from "zod";
import {
  NodeIdSchema,
  DurationSSchema,
  WindowSSchema,
  SemanticPrimitiveKindSchema,
  PosePersonResultSchema,
} from "./common.js";

// ── §4.1 Presence ──────────────────────────────────────────────────────────

/** ruview.presence.now */
export const PresenceNowInputSchema = z.object({
  node_id: NodeIdSchema,
});

// ── §4.1 Vitals ───────────────────────────────────────────────────────────

/** ruview.vitals.get_breathing */
export const VitalsGetBreathingInputSchema = z.object({
  node_id: NodeIdSchema,
  window_s: WindowSSchema,
});

/** ruview.vitals.get_heart_rate */
export const VitalsGetHeartRateInputSchema = z.object({
  node_id: NodeIdSchema,
  window_s: WindowSSchema,
});

/** ruview.vitals.get_all */
export const VitalsGetAllInputSchema = z.object({
  node_id: NodeIdSchema,
});

// ── §4.1 Pose ─────────────────────────────────────────────────────────────

/** ruview.pose.latest */
export const PoseLatestInputSchema = z.object({
  node_id: NodeIdSchema,
});

/** ruview.pose.subscribe */
export const PoseSubscribeInputSchema = z.object({
  node_id: NodeIdSchema,
  duration_s: DurationSSchema,
  callback_url: z
    .string()
    .url()
    .optional()
    .describe("Webhook URL to receive PoseDataMessage events (optional)."),
});

// ── §4.1 Primitives ───────────────────────────────────────────────────────

/** ruview.primitives.get */
export const PrimitivesGetInputSchema = z.object({
  node_id: NodeIdSchema,
  primitive: SemanticPrimitiveKindSchema,
});

/** ruview.primitives.list_active */
export const PrimitivesListActiveInputSchema = z.object({
  node_id: NodeIdSchema,
});

/** ruview.primitives.subscribe */
export const PrimitivesSubscribeInputSchema = z.object({
  node_id: NodeIdSchema,
  primitive: SemanticPrimitiveKindSchema.optional().describe(
    "Subscribe to a specific primitive. Omit to receive all active primitives."
  ),
  duration_s: DurationSSchema,
});

// ── §4.1 BFLD ────────────────────────────────────────────────────────────

/** ruview.bfld.last_scan */
export const BfldLastScanInputSchema = z.object({
  node_id: NodeIdSchema,
});

/** ruview.bfld.subscribe */
export const BfldSubscribeInputSchema = z.object({
  node_id: NodeIdSchema,
  duration_s: DurationSSchema,
});

// ── §4.1 Node ────────────────────────────────────────────────────────────

/** ruview.node.list — empty input per ADR-124 §4.1 table */
export const NodeListInputSchema = z.object({});

/** ruview.node.status */
export const NodeStatusInputSchema = z.object({
  node_id: z.string().min(1).describe("Node id to query status for."),
});

// ── §4.1 Vector ──────────────────────────────────────────────────────────

/** ruview.vector.search_pose */
export const VectorSearchPoseInputSchema = z.object({
  query_embedding: z
    .array(z.number())
    .min(1)
    .describe("Dense embedding vector to query against the HNSW index."),
  k: z
    .number()
    .int()
    .positive()
    .max(100)
    .optional()
    .default(10)
    .describe("Number of nearest neighbours to return (default 10, max 100)."),
  node_id: NodeIdSchema,
});

/** ruview.vector.store_pose */
export const VectorStorePoseInputSchema = z.object({
  pose: PosePersonResultSchema,
  node_id: z.string().min(1).describe("Node id that observed this pose."),
});

// ── §4.1a Policy / governance tools ──────────────────────────────────────

/** ruview.policy.can_access_vitals */
export const PolicyCanAccessVitalsInputSchema = z.object({
  agent_id: z.string().min(1).describe("Calling agent identifier."),
  node_id: z.string().min(1).describe("Target sensing node."),
  vital: z
    .enum(["breathing", "heart_rate", "all"])
    .describe("Which vital the agent is requesting."),
});

/** ruview.policy.can_query_presence */
export const PolicyCanQueryPresenceInputSchema = z.object({
  agent_id: z.string().min(1),
  scope: z
    .enum(["node", "fleet"])
    .describe("node = single node; fleet = all nodes / aggregated count."),
  node_id: NodeIdSchema,
  zone: z
    .string()
    .optional()
    .describe("Named zone within a node (e.g. 'living_room')."),
});

/** ruview.policy.can_subscribe */
export const PolicyCanSubscribeInputSchema = z.object({
  agent_id: z.string().min(1),
  topic: z
    .string()
    .min(1)
    .describe("MQTT topic or tool name the agent wishes to subscribe to."),
  duration_s: DurationSSchema,
});

/** ruview.policy.redact_identity_fields */
export const PolicyRedactIdentityFieldsInputSchema = z.object({
  payload: z.record(z.string(), z.unknown()).describe("Tool return value to redact."),
  agent_id: z.string().min(1),
});

/** ruview.policy.audit_log */
export const PolicyAuditLogInputSchema = z.object({
  agent_id: z.string().optional().describe("Filter to a specific agent."),
  since_ts: z
    .number()
    .optional()
    .describe("Return events after this Unix timestamp (ms)."),
});

// ── Catalog ───────────────────────────────────────────────────────────────

/**
 * Single source of truth: every tool name in the ADR-124 §4.1 + §4.1a catalog.
 * The schema-coverage test asserts this list exactly matches the exported schemas.
 */
export const TOOL_NAMES = [
  // §4.1 — 15 sensing tools
  "ruview.presence.now",
  "ruview.vitals.get_breathing",
  "ruview.vitals.get_heart_rate",
  "ruview.vitals.get_all",
  "ruview.pose.latest",
  "ruview.pose.subscribe",
  "ruview.primitives.get",
  "ruview.primitives.list_active",
  "ruview.primitives.subscribe",
  "ruview.bfld.last_scan",
  "ruview.bfld.subscribe",
  "ruview.node.list",
  "ruview.node.status",
  "ruview.vector.search_pose",
  "ruview.vector.store_pose",
  // §4.1a — 5 policy tools
  "ruview.policy.can_access_vitals",
  "ruview.policy.can_query_presence",
  "ruview.policy.can_subscribe",
  "ruview.policy.redact_identity_fields",
  "ruview.policy.audit_log",
] as const;

export type ToolName = (typeof TOOL_NAMES)[number];

/**
 * Map from tool name → its Zod input schema. Used by the MCP server's
 * CallTool handler for uniform schema-validation before dispatch.
 */
export const TOOL_INPUT_SCHEMAS: Record<ToolName, z.ZodTypeAny> = {
  "ruview.presence.now": PresenceNowInputSchema,
  "ruview.vitals.get_breathing": VitalsGetBreathingInputSchema,
  "ruview.vitals.get_heart_rate": VitalsGetHeartRateInputSchema,
  "ruview.vitals.get_all": VitalsGetAllInputSchema,
  "ruview.pose.latest": PoseLatestInputSchema,
  "ruview.pose.subscribe": PoseSubscribeInputSchema,
  "ruview.primitives.get": PrimitivesGetInputSchema,
  "ruview.primitives.list_active": PrimitivesListActiveInputSchema,
  "ruview.primitives.subscribe": PrimitivesSubscribeInputSchema,
  "ruview.bfld.last_scan": BfldLastScanInputSchema,
  "ruview.bfld.subscribe": BfldSubscribeInputSchema,
  "ruview.node.list": NodeListInputSchema,
  "ruview.node.status": NodeStatusInputSchema,
  "ruview.vector.search_pose": VectorSearchPoseInputSchema,
  "ruview.vector.store_pose": VectorStorePoseInputSchema,
  "ruview.policy.can_access_vitals": PolicyCanAccessVitalsInputSchema,
  "ruview.policy.can_query_presence": PolicyCanQueryPresenceInputSchema,
  "ruview.policy.can_subscribe": PolicyCanSubscribeInputSchema,
  "ruview.policy.redact_identity_fields": PolicyRedactIdentityFieldsInputSchema,
  "ruview.policy.audit_log": PolicyAuditLogInputSchema,
};
