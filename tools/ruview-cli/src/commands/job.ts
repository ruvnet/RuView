/**
 * ruview job — Job management commands.
 *
 * job status --id <job_id>  — poll a background training job.
 */

import type { Argv } from "yargs";
import path from "node:path";
import { existsSync, statSync, readSync, openSync, closeSync } from "node:fs";
import { loadConfig } from "../config.js";

export interface JobStatusResult {
  ok: boolean;
  job_id?: string;
  status?: "running" | "done" | "failed";
  log_path?: string;
  recent_log?: string[];
  error?: string;
}

/** Read the last `maxLines` lines of a file without loading the whole log (ADR-264 F6/O6). */
export function tailLines(filePath: string, maxLines = 20, maxBytes = 64 * 1024): string[] {
  const size = statSync(filePath).size;
  const start = Math.max(0, size - maxBytes);
  const buf = Buffer.alloc(size - start);
  const fd = openSync(filePath, "r");
  try {
    readSync(fd, buf, 0, buf.length, start);
  } finally {
    closeSync(fd);
  }
  const lines = buf.toString("utf8").split("\n");
  return lines.slice(Math.max(0, lines.length - maxLines));
}

/** Derive status from the recent log content. */
export function deriveJobStatus(lines: string[]): "running" | "done" | "failed" {
  for (let i = lines.length - 1; i >= 0; i--) {
    const line = lines[i]?.trim() ?? "";
    if (line === "# exit code: 0") {
      return "done";
    }
    if (line.startsWith("# exit code:") || line.startsWith("# ERROR:")) {
      return "failed";
    }
  }
  return "running";
}

/** Retrieve and derive the current status of a background training job. */
export function getJobStatus(jobId: string, jobsDir: string): JobStatusResult {
  const logPath = path.join(jobsDir, `${jobId}.log`);

  if (!existsSync(logPath)) {
    return {
      ok: false,
      error:
        `Job ${jobId} not found at ${logPath}. ` +
        "The CLI process that started the job may have been restarted.",
    };
  }

  const recentLog = tailLines(logPath, 20);
  const status = deriveJobStatus(recentLog);

  return {
    ok: true,
    job_id: jobId,
    status,
    log_path: logPath,
    recent_log: recentLog,
  };
}

export function jobCommand(cli: Argv): void {
  cli.command(
    "job <action>",
    "Job management commands",
    (y) =>
      y
        .positional("action", {
          choices: ["status"] as const,
          description: "Action to perform",
        })
        .option("id", {
          type: "string",
          demandOption: true,
          description: "Job ID returned by ruview train count",
        }),
    async (args) => {
      const config = loadConfig();

      if (args.action === "status") {
        const jobId = args.id as string;
        const result = getJobStatus(jobId, config.jobsDir);

        if (!result.ok) {
          process.stdout.write(JSON.stringify(result) + "\n");
          process.exit(0);
        }

        process.stdout.write(JSON.stringify(result, null, 2) + "\n");
      }
    }
  );
}
