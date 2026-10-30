/**
 * ruview train — Training commands.
 *
 * train count --paired <jsonl>  — kick off a count-cog training run.
 */

import type { Argv } from "yargs";
import { randomUUID } from "node:crypto";
import { mkdirSync, appendFileSync, openSync, closeSync } from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { loadConfig } from "../config.js";

export function trainCommand(cli: Argv): void {
  cli.command(
    "train <task>",
    "Training commands",
    (y) =>
      y
        .positional("task", {
          choices: ["count"] as const,
          description: "Which cog to train",
        })
        .option("paired", {
          type: "string",
          demandOption: true,
          description:
            "Path to the paired JSONL training file (produced by scripts/align-ground-truth.js)",
        })
        .option("epochs", {
          type: "number",
          default: 400,
          description: "Training epochs (default: 400)",
        })
        .option("lr", {
          type: "number",
          default: 1e-3,
          description: "Initial learning rate (default: 0.001)",
        })
        .option("output-dir", {
          type: "string",
          description: "Output directory for model artifacts",
        }),
    async (args) => {
      const config = loadConfig();
      const jobId = randomUUID();
      const logDir = config.jobsDir;
      mkdirSync(logDir, { recursive: true });
      const logPath = path.join(logDir, `${jobId}.log`);
      const queuedAt = Date.now() / 1000;

      const outputDir =
        (args["output-dir"] as string | undefined) ??
        "v2/crates/cog-person-count/cog/artifacts";

      const header = [
        `# RuView training job ${jobId}`,
        `# started: ${new Date().toISOString()}`,
        `# task: ${args.task}`,
        `# paired: ${args.paired}`,
        `# epochs: ${args.epochs}`,
        `# lr: ${args.lr}`,
        `# output-dir: ${outputDir}`,
        "",
      ].join("\n");
      appendFileSync(logPath, header);

      const logFdOut = openSync(logPath, "a");
      const logFdErr = openSync(logPath, "a");

      const cargoArgs = [
        "run",
        "--release",
        "-p",
        "wifi-densepose-train",
        "--",
        "--task",
        "count",
        "--paired",
        args.paired as string,
        "--epochs",
        String(args.epochs),
        "--lr",
        String(args.lr),
        "--output-dir",
        outputDir,
      ];

      const child = spawn("cargo", cargoArgs, {
        detached: true,
        stdio: ["ignore", logFdOut, logFdErr],
      });
      child.unref();

      // The child holds its own duplicates of the log fds; close the parent's
      // copies immediately to avoid leaking 2 fds per training job (ADR-264 F6/O6).
      closeSync(logFdOut);
      closeSync(logFdErr);

      child.on("error", (e) => {
        appendFileSync(logPath, `\n# ERROR: ${e.message}\n`);
      });
      child.on("close", (code) => {
        appendFileSync(logPath, `\n# exit code: ${code}\n`);
      });

      process.stdout.write(
        JSON.stringify(
          {
            ok: true,
            job_id: jobId,
            status: "running",
            log_path: logPath,
            queued_at: queuedAt,
            note: `Poll with: ruview job status --id ${jobId}`,
          },
          null,
          2
        ) + "\n"
      );
    }
  );
}
