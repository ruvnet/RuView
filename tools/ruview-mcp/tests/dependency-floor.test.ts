/**
 * Dependency floors for the published runtime, and the advertised tool
 * schema contract that the zod 4 move must not change.
 *
 * Floors (public upstream advisories):
 *   - zod < 4.6.6: memory exhaustion validating oversized constrained
 *     collections (GHSA-mrhx-c7xc-m3w5). Every tool call is parsed by zod.
 *   - @modelcontextprotocol/sdk <= 1.31.0: several SDK advisories, and its
 *     express/body-parser/proxy-addr/hono chain.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { zodInputJsonSchema } from "../src/json-schema.js";
import { registryListSchema } from "../src/tools/registry-list.js";
import { trainCountSchema } from "../src/tools/train-count.js";

function installedVersion(pkg: string): number[] {
  const p = resolve(process.cwd(), "node_modules", pkg, "package.json");
  const v = (JSON.parse(readFileSync(p, "utf-8")) as { version: string }).version;
  return v.split("-")[0].split(".").map(Number);
}

function atLeast(have: number[], want: number[]): boolean {
  for (let i = 0; i < want.length; i++) {
    if ((have[i] ?? 0) !== want[i]) return (have[i] ?? 0) > want[i];
  }
  return true;
}

const pkg = JSON.parse(
  readFileSync(resolve(process.cwd(), "package.json"), "utf-8")
) as { dependencies: Record<string, string> };

describe("dependency floors", () => {
  it("installs zod >= 4.6.6", () => {
    expect(atLeast(installedVersion("zod"), [4, 6, 6])).toBe(true);
  });

  it("installs @modelcontextprotocol/sdk >= 1.32.0", () => {
    expect(atLeast(installedVersion("@modelcontextprotocol/sdk"), [1, 32, 0])).toBe(true);
  });

  it("declares ranges that exclude the vulnerable versions", () => {
    expect(pkg.dependencies["zod"]).toBe("^4.6.7");
    expect(pkg.dependencies["@modelcontextprotocol/sdk"]).toBe("^1.32.2");
  });
});

describe("advertised input schema", () => {
  it("keeps plain objects closed", () => {
    const s = zodInputJsonSchema(registryListSchema);
    expect(s["type"]).toBe("object");
    expect(s["additionalProperties"]).toBe(false);
    expect(s["$schema"]).toBeUndefined();
  });

  it("leaves defaulted fields optional", () => {
    const required = (zodInputJsonSchema(trainCountSchema)["required"] ?? []) as string[];
    expect(required).not.toContain("epochs");
    expect(required).not.toContain("learning_rate");
    expect(zodInputJsonSchema(registryListSchema)["required"] ?? []).not.toContain("refresh");
  });
});
