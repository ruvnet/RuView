#!/usr/bin/env node
/**
 * @ruvnet/rvagent — RuView MCP Server
 *
 * Exposes RuView's WiFi-DensePose sensing capabilities as Model Context Protocol
 * (MCP) tools that Claude Code, Cursor, Codex, and other MCP-compatible agents
 * can call directly.
 *
 * Transports (ADR-264 O3):
 *   stdio (default)      node dist/index.js
 *   Streamable HTTP      RVAGENT_HTTP_PORT=3001 node dist/index.js
 *                        (127.0.0.1-bound, Origin-gated, optional bearer token —
 *                        see http-transport.ts for the security model)
 *
 * Tool naming (ADR-264 O4): canonical names are underscore-form
 * (host tool-name regexes commonly enforce ^[a-zA-Z0-9_-]{1,64}$). The
 * pre-ADR-264 dotted names (ruview.bfld.last_scan, …) remain callable as
 * router-only aliases for one deprecation cycle; tools/list advertises the
 * underscore form only.
 *
 * Validation (ADR-264 O5): each tool declares ONE Zod schema. The CallTool
 * gate parses exactly once and hands the typed result to the handler; the
 * advertised JSON Schema is generated from the same Zod source, so what is
 * advertised is what is enforced.
 *
 * To register with Claude Code:
 *   claude mcp add ruview -- npx -y @ruvnet/rvagent
 *
 * See ADR-104 for the original design rationale and ADR-264 for the npm
 * deep-review this layout implements.
 */

import { createRequire } from "node:module";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { argv } from "node:process";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  McpError,
  ErrorCode,
} from "@modelcontextprotocol/sdk/types.js";
import type { z } from "zod";

import { zodInputJsonSchema } from "./json-schema.js";
import { loadConfig } from "./config.js";
import { csiLatestSchema, csiLatest } from "./tools/csi-latest.js";
import { poseInferSchema, poseInfer } from "./tools/pose-infer.js";
import { countInferSchema, countInfer } from "./tools/count-infer.js";
import { registryListSchema, registryList } from "./tools/registry-list.js";
import {
  trainCountSchema,
  trainCount,
  jobStatusSchema,
  jobStatus,
} from "./tools/train-count.js";
import { bfldLastScanSchema, bfldLastScan } from "./tools/bfld-last-scan.js";
import { bfldSubscribeSchema, bfldSubscribe } from "./tools/bfld-subscribe.js";
import { presenceNowSchema, presenceNow } from "./tools/presence-now.js";
import {
  vitalsGetBreathingSchema,
  vitalsGetBreathing,
} from "./tools/vitals-get-breathing.js";
import {
  vitalsGetHeartRateSchema,
  vitalsGetHeartRate,
} from "./tools/vitals-get-heart-rate.js";
import { vitalsGetAllSchema, vitalsGetAll } from "./tools/vitals-get-all.js";
// NOTE: ./http-transport.js is imported lazily in main() — it chain-loads the
// SDK's streamableHttp module (~48 ms MEASURED), which the default stdio path
// never uses.

// Single-source the version from package.json (ADR-264 O8/ADR-265 D3).
const require = createRequire(import.meta.url);
const PACKAGE_VERSION: string = (
  require("../package.json") as { version: string }
).version;
const SERVER_NAME = "rvagent";

// ── Tool registry ──────────────────────────────────────────────────────────

type RuviewConfig = ReturnType<typeof loadConfig>;

interface ToolDef {
  name: string;
  description: string;
  /** The single validation source; the advertised JSON Schema derives from it. */
  schema: z.ZodTypeAny;
  handler: (parsedArgs: unknown, config: RuviewConfig) => Promise<object>;
}

export const TOOLS: ToolDef[] = [
  {
    name: "ruview_csi_latest",
    description:
      "Pull the latest CSI window from a running wifi-densepose-sensing-server. " +
      "Returns 56-subcarrier × 20-frame amplitude/phase arrays suitable for " +
      "downstream inference or research analysis.",
    schema: csiLatestSchema,
    handler: (args, config) =>
      csiLatest(args as Parameters<typeof csiLatest>[0], config),
  },
  {
    name: "ruview_pose_infer",
    description:
      "Run a single-shot 17-keypoint COCO pose estimation inference using the " +
      "cog-pose-estimation Cog binary (ADR-101). Accepts a CSI window JSON file " +
      "or uses the live sensing-server if no window is provided. " +
      "Returns [{keypoints: [[x,y]×17], confidence}] per detected person.",
    schema: poseInferSchema,
    handler: (args, config) =>
      poseInfer(args as Parameters<typeof poseInfer>[0], config),
  },
  {
    name: "ruview_count_infer",
    description:
      "Run a single-shot person-count inference using the cog-person-count Cog " +
      "binary (ADR-103). Returns {count, confidence, count_p95_low, count_p95_high} " +
      "with a Stoer-Wagner multi-node fusion upper bound when multiple nodes are active.",
    schema: countInferSchema,
    handler: (args, config) =>
      countInfer(args as Parameters<typeof countInfer>[0], config),
  },
  {
    name: "ruview_registry_list",
    description:
      "List cogs from the Cognitum edge module registry (ADR-102). " +
      "Fetches /api/v1/edge/registry from the sensing-server, which proxies the " +
      "canonical GCS catalog (105 cogs, 11 categories). Supports category filter and search.",
    schema: registryListSchema,
    handler: (args, config) =>
      registryList(args as Parameters<typeof registryList>[0], config),
  },
  {
    name: "ruview_train_count",
    description:
      "Kick off a cog-person-count training run using the Candle GPU trainer " +
      "(ADR-103). The paired JSONL file provides CSI windows + camera-derived " +
      "person-count labels. Returns a job_id to poll with ruview_job_status.",
    schema: trainCountSchema,
    handler: (args, config) =>
      trainCount(args as Parameters<typeof trainCount>[0], config),
  },
  {
    name: "ruview_job_status",
    description:
      "Poll the status of a background training job started by ruview_train_count. " +
      "Returns {status, epochs_done, epochs_total, recent_log} for the given job_id.",
    schema: jobStatusSchema,
    handler: (args, config) =>
      jobStatus(args as Parameters<typeof jobStatus>[0], config),
  },
  // ── ADR-124 BFLD tools (Phase 4 Refinement; underscore names per ADR-264) ─
  {
    name: "ruview_bfld_last_scan",
    description:
      "Return the most recent BFLD scan result for a node (ADR-118/ADR-121). " +
      "Fields: node_id, identity_risk_score [0,1], privacy_class, n_frames, timestamp_ms. " +
      "Proxied from sensing-server GET /api/v1/bfld/<node_id>/last_scan which aggregates " +
      "the MQTT state topics ruview/<node_id>/bfld/* (ADR-122 §2.2).",
    schema: bfldLastScanSchema,
    handler: (args, config) =>
      bfldLastScan(args as Parameters<typeof bfldLastScan>[0], config),
  },
  {
    name: "ruview_bfld_subscribe",
    description:
      "Subscribe to BFLD events on ruview/<node_id>/bfld/* for duration_s seconds (ADR-122). " +
      "Returns {ok, subscription_id, expires_at, topic}. When the sensing-server is unreachable, " +
      "returns a synthetic envelope with ok:false,warn:true so the caller can distinguish " +
      "a network error from an invalid request.",
    schema: bfldSubscribeSchema,
    handler: (args, config) =>
      bfldSubscribe(args as Parameters<typeof bfldSubscribe>[0], config),
  },
  // ── ADR-124 Presence + Vitals tools ───────────────────────────────────────
  {
    name: "ruview_presence_now",
    description:
      "Return current occupancy for a node: present, n_persons, confidence, timestamp_ms. " +
      "Wraps EdgeVitalsMessage.presence + n_persons (ADR-124 §4.1, ws.py:74-88).",
    schema: presenceNowSchema,
    handler: (args, config) =>
      presenceNow(args as Parameters<typeof presenceNow>[0], config),
  },
  {
    name: "ruview_vitals_get_breathing",
    description:
      "Return breathing rate for a node: breathing_rate_bpm (null if unavailable), " +
      "confidence, timestamp_ms. Wraps EdgeVitalsMessage.breathing_rate_bpm (ws.py:82).",
    schema: vitalsGetBreathingSchema,
    handler: (args, config) =>
      vitalsGetBreathing(args as Parameters<typeof vitalsGetBreathing>[0], config),
  },
  {
    name: "ruview_vitals_get_heart_rate",
    description:
      "Return heart rate for a node: heartrate_bpm (null if unavailable), " +
      "confidence, timestamp_ms. Wraps EdgeVitalsMessage.heartrate_bpm (ws.py:83).",
    schema: vitalsGetHeartRateSchema,
    handler: (args, config) =>
      vitalsGetHeartRate(args as Parameters<typeof vitalsGetHeartRate>[0], config),
  },
  {
    name: "ruview_vitals_get_all",
    description:
      "Return the full EdgeVitalsMessage for a node (all fields except raw): " +
      "presence, n_persons, confidence, breathing_rate_bpm, heartrate_bpm, motion, zone_id. " +
      "Full surface of ws.py:74-88.",
    schema: vitalsGetAllSchema,
    handler: (args, config) =>
      vitalsGetAll(args as Parameters<typeof vitalsGetAll>[0], config),
  },
];

/**
 * Pre-ADR-264 dotted tool names, accepted at call time for one deprecation
 * cycle. Router-only: tools/list never advertises these.
 */
export const TOOL_ALIASES: Record<string, string> = {
  "ruview.bfld.last_scan": "ruview_bfld_last_scan",
  "ruview.bfld.subscribe": "ruview_bfld_subscribe",
  "ruview.presence.now": "ruview_presence_now",
  "ruview.vitals.get_breathing": "ruview_vitals_get_breathing",
  "ruview.vitals.get_heart_rate": "ruview_vitals_get_heart_rate",
  "ruview.vitals.get_all": "ruview_vitals_get_all",
};

/**
 * Advertised JSON Schema, generated from the Zod source (ADR-264 O5).
 * Memoized: schemas are static for the process lifetime, and tools/list is
 * called once per session (per HTTP session under the session-per-server
 * model) — no point re-walking the Zod tree each time.
 */
const jsonSchemaCache = new Map<string, object>();
export function toolInputJsonSchema(def: ToolDef): object {
  const cached = jsonSchemaCache.get(def.name);
  if (cached !== undefined) return cached;
  const raw = zodInputJsonSchema(def.schema);
  jsonSchemaCache.set(def.name, raw);
  return raw;
}

// ── Server factory ──────────────────────────────────────────────────────────

/**
 * Build a fully-wired MCP Server. A factory (not a singleton) because each
 * Streamable-HTTP session needs its own Server instance (ADR-264 F7/O3).
 */
export function buildServer(config: RuviewConfig = loadConfig()): Server {
  const server = new Server(
    { name: SERVER_NAME, version: PACKAGE_VERSION },
    { capabilities: { tools: {} } }
  );

  server.setRequestHandler(ListToolsRequestSchema, () => ({
    tools: TOOLS.map((t) => ({
      name: t.name,
      description: t.description,
      inputSchema: toolInputJsonSchema(t),
    })),
  }));

  // Call tool handler — the SINGLE Zod validation gate (ADR-264 O5): parse
  // once, hand the typed result (with defaults applied) to the handler.
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name: rawName, arguments: args } = request.params;
    const name = TOOL_ALIASES[rawName] ?? rawName;
    const tool = TOOLS.find((t) => t.name === name);

    if (!tool) {
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify({
              ok: false,
              error: `Unknown tool "${rawName}". Available tools: ${TOOLS.map((t) => t.name).join(", ")}`,
            }),
          },
        ],
        isError: true,
      };
    }

    const parsed = tool.schema.safeParse(args ?? {});
    if (!parsed.success) {
      throw new McpError(
        ErrorCode.InvalidParams,
        `Invalid arguments for tool "${rawName}": ${parsed.error.message}`
      );
    }

    try {
      const result = await tool.handler(parsed.data, config);
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify(result, null, 2),
          },
        ],
      };
    } catch (e: unknown) {
      if (e instanceof McpError) throw e; // propagate typed errors unchanged
      const message = e instanceof Error ? e.message : String(e);
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify({
              ok: false,
              error: message,
            }),
          },
        ],
        isError: true,
      };
    }
  });

  return server;
}

// ── Server bootstrap ────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const config = loadConfig();

  // stdio transport (default, always on).
  const stdioServer = buildServer(config);
  const transport = new StdioServerTransport();
  await stdioServer.connect(transport);

  // Streamable HTTP transport — explicit opt-in only (ADR-264 O3). Lazily
  // imported so the stdio path never pays the streamableHttp load cost.
  const httpPort = process.env["RVAGENT_HTTP_PORT"];
  let httpNote = "";
  if (httpPort !== undefined && httpPort !== "") {
    const { createHttpTransport } = await import("./http-transport.js");
    const { boundAddress } = await createHttpTransport(
      () => buildServer(config),
      { port: Number(httpPort) }
    );
    httpNote = ` HTTP: ${boundAddress}/mcp.`;
  }

  // Log to stderr so it doesn't interfere with the MCP stdio protocol.
  process.stderr.write(
    `[@ruvnet/rvagent] Server v${PACKAGE_VERSION} started. ` +
      `Sensing server: ${config.sensingServerUrl}.${httpNote}\n`
  );
}

// CLI guard: boot the server only when this module is the entrypoint — invoked
// as the `rvagent` / `ruview-mcp` bin or `node dist/index.js`. Importing it as a
// library (`import { buildServer } from "@ruvnet/rvagent"`) must NOT side-effect
// connect a StdioServerTransport to the consumer's stdin/stdout. Realpath both
// sides because npm's bin shim is a symlink and passes a non-normalized,
// possibly case-skewed argv[1] on Windows (mirrors harness/ruview/bin/cli.js).
const invokedDirectly = (() => {
  if (!argv[1]) return false;
  try {
    const a = realpathSync(argv[1]);
    const b = realpathSync(fileURLToPath(import.meta.url));
    return process.platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b;
  } catch {
    return false;
  }
})();

if (invokedDirectly) {
  main().catch((e) => {
    process.stderr.write(`[ruview-mcp] Fatal: ${String(e)}\n`);
    process.exit(1);
  });
}
