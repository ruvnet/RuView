import { describe, it, expect, beforeEach, afterAll } from "@jest/globals";
import { loadConfig } from "../src/config.js";

describe("loadConfig()", () => {
  const originalEnv = process.env;

  beforeEach(() => {
    process.env = { ...originalEnv };
  });

  afterAll(() => {
    process.env = originalEnv;
  });

  it("loads default configuration when environment variables are unset", () => {
    delete process.env["RUVIEW_SENSING_SERVER_URL"];
    delete process.env["RUVIEW_API_TOKEN"];
    delete process.env["RUVIEW_POSE_COG_BINARY"];
    delete process.env["RUVIEW_COUNT_COG_BINARY"];
    delete process.env["RUVIEW_JOBS_DIR"];

    const config = loadConfig();
    expect(config.sensingServerUrl).toBe("http://localhost:3000");
    expect(config.apiToken).toBeUndefined();
    expect(config.poseCogBinary).toBe("cog-pose-estimation");
    expect(config.countCogBinary).toBe("cog-person-count");
    expect(config.jobsDir).toContain(".ruview");
  });

  it("overrides default values with environment variables", () => {
    process.env["RUVIEW_SENSING_SERVER_URL"] = "http://sensing.local:8080";
    process.env["RUVIEW_API_TOKEN"] = "secret-token-123";
    process.env["RUVIEW_POSE_COG_BINARY"] = "/opt/bin/pose";
    process.env["RUVIEW_COUNT_COG_BINARY"] = "/opt/bin/count";
    process.env["RUVIEW_JOBS_DIR"] = "/tmp/ruview-jobs";

    const config = loadConfig();
    expect(config.sensingServerUrl).toBe("http://sensing.local:8080");
    expect(config.apiToken).toBe("secret-token-123");
    expect(config.poseCogBinary).toBe("/opt/bin/pose");
    expect(config.countCogBinary).toBe("/opt/bin/count");
    expect(config.jobsDir).toBe("/tmp/ruview-jobs");
  });
});
