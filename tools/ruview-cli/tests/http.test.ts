import { describe, it, expect, afterAll, beforeAll } from "@jest/globals";
import * as http from "node:http";
import type { AddressInfo } from "node:net";
import { ok, err, sensingGet } from "../src/http.js";

describe("HTTP client helpers (tools/ruview-cli)", () => {
  describe("ok() and err()", () => {
    it("creates ok result tuple", () => {
      const res = ok({ count: 5 });
      expect(res).toEqual({ ok: true, data: { count: 5 } });
    });

    it("creates err result tuple", () => {
      const res = err("connection refused");
      expect(res).toEqual({ ok: false, error: "connection refused" });
    });
  });

  describe("sensingGet()", () => {
    let server: http.Server;
    let baseUrl: string;
    let lastAuthHeader: string | undefined;

    beforeAll(async () => {
      server = http.createServer((req, res) => {
        lastAuthHeader = req.headers["authorization"];

        if (req.url === "/api/v1/test/ok") {
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ status: "healthy", items: [1, 2, 3] }));
          return;
        }

        if (req.url === "/api/v1/test/non-json") {
          res.writeHead(200, { "Content-Type": "text/plain" });
          res.end("not-json-content");
          return;
        }

        if (req.url === "/api/v1/test/error") {
          res.writeHead(503, { "Content-Type": "text/plain" });
          res.end("service unavailable");
          return;
        }

        res.writeHead(404, { "Content-Type": "text/plain" });
        res.end("not found");
      });

      await new Promise<void>((resolve) => {
        server.listen(0, "127.0.0.1", () => resolve());
      });

      const addr = server.address() as AddressInfo;
      baseUrl = `http://127.0.0.1:${addr.port}`;
    });

    afterAll(async () => {
      await new Promise<void>((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      });
    });

    it("parses valid JSON response on 200", async () => {
      const res = await sensingGet<{ status: string; items: number[] }>(
        baseUrl,
        "/api/v1/test/ok",
        undefined
      );
      expect(res.ok).toBe(true);
      if (res.ok) {
        expect(res.data.status).toBe("healthy");
        expect(res.data.items).toEqual([1, 2, 3]);
      }
    });

    it("passes Bearer token Authorization header when configured", async () => {
      await sensingGet(baseUrl, "/api/v1/test/ok", "test-secret-token");
      expect(lastAuthHeader).toBe("Bearer test-secret-token");
    });

    it("omits Authorization header when token is undefined", async () => {
      await sensingGet(baseUrl, "/api/v1/test/ok", undefined);
      expect(lastAuthHeader).toBeUndefined();
    });

    it("returns error result on HTTP non-200 status", async () => {
      const res = await sensingGet(baseUrl, "/api/v1/test/error", undefined);
      expect(res.ok).toBe(false);
      if (!res.ok) {
        expect(res.error).toContain("HTTP 503");
        expect(res.error).toContain("service unavailable");
      }
    });

    it("returns error result on non-JSON response", async () => {
      const res = await sensingGet(baseUrl, "/api/v1/test/non-json", undefined);
      expect(res.ok).toBe(false);
      if (!res.ok) {
        expect(res.error).toContain("Non-JSON response");
      }
    });

    it("returns network error when host is unreachable", async () => {
      // Connect to unused port
      const res = await sensingGet("http://127.0.0.1:19999", "/api/v1/test", undefined);
      expect(res.ok).toBe(false);
      if (!res.ok) {
        expect(res.error).toContain("Network error");
      }
    });
  });
});
