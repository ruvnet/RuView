import { describe, it, expect, beforeEach, afterEach } from "@jest/globals";
import os from "node:os";
import path from "node:path";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tailLines, deriveJobStatus, getJobStatus } from "../src/commands/job.js";

describe("Job status helpers (ADR-264 F6/O6)", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = mkdtempSync(path.join(os.tmpdir(), "ruview-job-test-"));
  });

  afterEach(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  describe("tailLines()", () => {
    it("returns all lines when file has fewer than maxLines", () => {
      const filePath = path.join(tmpDir, "short.log");
      writeFileSync(filePath, "line 1\nline 2\nline 3\n");
      const lines = tailLines(filePath, 20);
      expect(lines).toEqual(["line 1", "line 2", "line 3", ""]);
    });

    it("bounds output to the last maxLines lines", () => {
      const filePath = path.join(tmpDir, "numbered.log");
      const allLines = Array.from({ length: 50 }, (_, i) => `line ${i + 1}`).join("\n");
      writeFileSync(filePath, allLines);
      const lines = tailLines(filePath, 10);
      expect(lines).toHaveLength(10);
      expect(lines[0]).toBe("line 41");
      expect(lines[9]).toBe("line 50");
    });

    it("handles large files via bounded byte window", () => {
      const filePath = path.join(tmpDir, "large.log");
      // Create ~100 KB file (exceeds default maxBytes of 64 KB)
      const chunk = "epoch 100/400 step 20/50 loss: 0.2345 lr: 0.001\n";
      writeFileSync(filePath, chunk.repeat(2500));
      const lines = tailLines(filePath, 5);
      expect(lines).toHaveLength(5);
      expect(lines[0]).toContain("epoch 100/400");
    });

    it("handles empty files gracefully", () => {
      const filePath = path.join(tmpDir, "empty.log");
      writeFileSync(filePath, "");
      const lines = tailLines(filePath, 20);
      expect(lines).toEqual([""]);
    });
  });

  describe("deriveJobStatus()", () => {
    it("returns 'running' when log has neither exit code nor error", () => {
      const lines = [
        "# RuView training job 123",
        "epoch 1/400 loss: 1.23",
        "epoch 2/400 loss: 0.98",
      ];
      expect(deriveJobStatus(lines)).toBe("running");
    });

    it("returns 'done' when log ends with '# exit code: 0'", () => {
      const lines = [
        "epoch 400/400 loss: 0.05",
        "",
        "# exit code: 0",
        "",
      ];
      expect(deriveJobStatus(lines)).toBe("done");
    });

    it("returns 'failed' when log contains an error marker", () => {
      const lines = [
        "# RuView training job 123",
        "",
        "# ERROR: spawn cargo ENOENT",
        "",
        "# exit code: -4058",
      ];
      expect(deriveJobStatus(lines)).toBe("failed");
    });

    it("returns 'failed' when exit code is non-zero", () => {
      const lines = [
        "training failed due to out-of-memory",
        "# exit code: 1",
      ];
      expect(deriveJobStatus(lines)).toBe("failed");
    });
  });

  describe("getJobStatus()", () => {
    it("returns ok: false when job log does not exist", () => {
      const res = getJobStatus("nonexistent-uuid", tmpDir);
      expect(res.ok).toBe(false);
      expect(res.error).toContain("Job nonexistent-uuid not found");
      expect(res.error).toContain("restarted");
    });

    it("returns job status and recent log lines when job log exists", () => {
      const jobId = "job-456";
      const logPath = path.join(tmpDir, `${jobId}.log`);
      writeFileSync(
        logPath,
        [
          `# RuView training job ${jobId}`,
          "# task: count",
          "epoch 1/100 loss: 0.5",
          "",
          "# exit code: 0",
        ].join("\n")
      );

      const res = getJobStatus(jobId, tmpDir);
      expect(res.ok).toBe(true);
      expect(res.job_id).toBe(jobId);
      expect(res.status).toBe("done");
      expect(res.log_path).toBe(logPath);
      expect(res.recent_log).toBeDefined();
      expect(res.recent_log?.join("\n")).toContain("# exit code: 0");
    });
  });
});
