/**
 * Zod -> advertised JSON Schema for MCP tools/list (ADR-264 O5).
 *
 * Kept free of import.meta so the test suite can import it directly.
 */
import { z } from "zod";

/**
 * Input-side JSON Schema for a tool's Zod schema.
 *
 * `io: "input"` keeps defaulted fields optional, which is what a client may
 * omit. Plain (strip) objects are advertised as closed, matching what
 * zod-to-json-schema emitted before the zod 4 move.
 */
export function zodInputJsonSchema(schema: z.ZodTypeAny): Record<string, unknown> {
  const raw = z.toJSONSchema(schema, {
    target: "draft-7",
    reused: "inline",
    io: "input",
    override: (ctx) => {
      const zdef = (ctx.zodSchema as { _zod?: { def?: { type?: string } } })._zod?.def;
      if (zdef?.type === "object" && ctx.jsonSchema.additionalProperties === undefined) {
        ctx.jsonSchema.additionalProperties = false;
      }
    },
  }) as Record<string, unknown>;
  delete raw["$schema"];
  return raw;
}
