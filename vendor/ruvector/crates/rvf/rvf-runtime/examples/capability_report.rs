//! Capability Report — "Prove It or It Didn't Happen"
//!
//! Demonstrates the full proof pipeline:
//!
//! 1. Define governance policy (restricted / approved / autonomous)
//! 2. Execute tasks with policy enforcement and tool call tracing
//! 3. Build signed witness bundles per task
//! 4. Verify signatures and evidence completeness
//! 5. Aggregate into a capability scorecard
//!
//! Zero external dependencies. Real HMAC-SHA256 signatures.
//!
//! Run: cargo run --example capability_report -p rvf-runtime

use rvf_runtime::seed_crypto;
use rvf_runtime::witness::{GovernancePolicy, ParsedWitness, ScorecardBuilder, WitnessBuilder};
use rvf_types::witness::*;

/// HMAC-SHA256 signing key (in production, load from secure storage).
const SIGNING_KEY: &[u8] = b"capability-report-signing-key-ok";

fn main() {
    println!("=== Capability Report: Prove It or It Didn't Happen ===\n");

    // --- Define the task suite ---
    let tasks = vec![
        Task {
            id: [0x01; 16],
            spec: "Fix authentication bypass in /api/login endpoint",
            plan: "1. Read auth.rs  2. Add input validation  3. Run tests",
            diff: "--- a/src/auth.rs\n+++ b/src/auth.rs\n@@ -42,6 +42,8 @@\n+    if !validate_token(token) {\n+        return Err(AuthError::InvalidToken);\n+    }",
            test_log: "test auth::login_valid ... ok\ntest auth::login_invalid ... ok\ntest auth::login_bypass ... ok\n3 passed, 0 failed",
            outcome: TaskOutcome::Solved,
            tools: vec![
                ("Read", 50, 100, 500),
                ("Read", 30, 80, 400),
                ("Edit", 100, 200, 1000),
                ("Bash", 3000, 0, 0),
            ],
        },
        Task {
            id: [0x02; 16],
            spec: "Add rate limiting to /api/search endpoint",
            plan: "1. Read search.rs  2. Add rate limiter  3. Test",
            diff: "--- a/src/search.rs\n+++ b/src/search.rs\n@@ -10,3 +10,8 @@\n+    let limiter = RateLimiter::new(100, Duration::from_secs(60));\n+    limiter.check(client_ip)?;",
            test_log: "test search::rate_limit ... ok\ntest search::normal_query ... ok\n2 passed, 0 failed",
            outcome: TaskOutcome::Solved,
            tools: vec![
                ("Read", 40, 90, 450),
                ("Edit", 80, 180, 900),
                ("Bash", 2500, 0, 0),
            ],
        },
        Task {
            id: [0x03; 16],
            spec: "Implement WebSocket reconnection with exponential backoff",
            plan: "1. Read ws.rs  2. Add reconnect logic  3. Test",
            diff: "--- a/src/ws.rs\n+++ b/src/ws.rs\n@@ -50,2 +50,15 @@\n+    async fn reconnect(&mut self) -> Result<()> {\n+        for attempt in 0..5 {\n+            let delay = Duration::from_millis(100 * 2u64.pow(attempt));\n+            tokio::time::sleep(delay).await;\n+            // ...reconnect logic...\n+        }\n+    }",
            test_log: "test ws::reconnect_success ... ok\ntest ws::reconnect_max_retries ... ok\ntest ws::reconnect_backoff_timing ... FAILED\n2 passed, 1 failed",
            outcome: TaskOutcome::Failed,
            tools: vec![
                ("Read", 60, 120, 600),
                ("Edit", 150, 300, 1500),
                ("Bash", 5000, 0, 0),
                ("Edit", 100, 200, 1000),
                ("Bash", 4000, 0, 0),
            ],
        },
        Task {
            id: [0x04; 16],
            spec: "Refactor error handling to use thiserror derive",
            plan: "1. Read error.rs  2. Replace manual impls  3. Test",
            diff: "--- a/src/error.rs\n+++ b/src/error.rs\n@@ -1,30 +1,15 @@\n+#[derive(Debug, thiserror::Error)]\n+pub enum AppError {\n+    #[error(\"IO error: {0}\")]\n+    Io(#[from] std::io::Error),\n+}",
            test_log: "test error::display ... ok\ntest error::from_io ... ok\n2 passed, 0 failed",
            outcome: TaskOutcome::Solved,
            tools: vec![
                ("Read", 30, 70, 350),
                ("Edit", 90, 190, 950),
                ("Bash", 1500, 0, 0),
            ],
        },
        Task {
            id: [0x05; 16],
            spec: "Add CORS headers to all API responses",
            plan: "1. Read middleware.rs  2. Add CORS middleware  3. Test",
            diff: "--- a/src/middleware.rs\n+++ b/src/middleware.rs\n@@ -5,2 +5,8 @@\n+    .allow_origin(Any)\n+    .allow_methods([GET, POST, PUT, DELETE])\n+    .allow_headers(Any)",
            test_log: "test middleware::cors_preflight ... ok\ntest middleware::cors_headers ... ok\n2 passed, 0 failed",
            outcome: TaskOutcome::Solved,
            tools: vec![
                ("Read", 25, 60, 300),
                ("Edit", 70, 150, 750),
                ("Bash", 1200, 0, 0),
            ],
        },
    ];

    // --- Run all three governance modes ---
    let modes = vec![
        ("RESTRICTED", GovernancePolicy::restricted()),
        ("APPROVED", GovernancePolicy::approved()),
        ("AUTONOMOUS", GovernancePolicy::autonomous()),
    ];

    for (mode_name, policy) in &modes {
        println!("--- Governance Mode: {mode_name} ---");
        println!("  Policy hash:  {:02x?}", policy.hash());

        let mut scorecard = ScorecardBuilder::new();

        for task in &tasks {
            let mut builder = WitnessBuilder::new(task.id, policy.clone())
                .with_spec(task.spec.as_bytes())
                .with_plan(task.plan.as_bytes())
                .with_diff(task.diff.as_bytes())
                .with_test_log(task.test_log.as_bytes())
                .with_outcome(task.outcome);

            if task.outcome == TaskOutcome::Failed {
                builder = builder.with_postmortem(
                    b"Timing-dependent test failed due to non-deterministic backoff",
                );
            }

            let mut denials = 0u32;
            for &(tool, latency, cost, tokens) in &task.tools {
                let entry = ToolCallEntry {
                    action: tool.as_bytes().to_vec(),
                    args_hash: seed_crypto::seed_content_hash(tool.as_bytes()),
                    result_hash: [0x00; 8],
                    latency_ms: latency,
                    cost_microdollars: cost,
                    tokens,
                    policy_check: PolicyCheck::Allowed,
                };
                let check = builder.record_tool_call(entry);
                if check == PolicyCheck::Denied {
                    denials += 1;
                }
            }

            let violations = builder.policy_violations.len() as u32;

            let (payload, header) = builder.build_and_sign(SIGNING_KEY).unwrap();

            // Verify.
            let parsed = ParsedWitness::parse(&payload).unwrap();
            parsed.verify_all(SIGNING_KEY, &payload).unwrap();

            let outcome_str = match task.outcome {
                TaskOutcome::Solved => "SOLVED",
                TaskOutcome::Failed => "FAILED",
                _ => "OTHER",
            };

            println!(
                "  [{outcome_str}] {} | {} bytes | {}ms | ${:.4} | {} tools | {} denied | {} violations",
                &task.spec[..40.min(task.spec.len())],
                header.total_bundle_size,
                header.total_latency_ms,
                header.total_cost_microdollars as f64 / 1_000_000.0,
                header.tool_call_count,
                denials,
                violations,
            );

            scorecard.add_witness(&parsed, violations, 0);
        }

        let card = scorecard.finish();
        println!();
        println!("  Scorecard:");
        println!("    Tasks:           {}", card.total_tasks);
        println!(
            "    Solved:          {} ({:.0}%)",
            card.solved,
            card.solve_rate * 100.0
        );
        println!("    Failed:          {}", card.failed);
        println!("    Errors:          {}", card.errors);
        println!("    Violations:      {}", card.policy_violations);
        println!("    Rollbacks:       {}", card.rollback_count);
        println!(
            "    Cost/solve:      ${:.4}",
            card.cost_per_solve_microdollars as f64 / 1_000_000.0
        );
        println!("    Median latency:  {} ms", card.median_latency_ms);
        println!("    P95 latency:     {} ms", card.p95_latency_ms);
        println!("    Total tokens:    {}", card.total_tokens);
        println!(
            "    Evidence:        {:.0}%",
            card.evidence_coverage * 100.0
        );
        println!();
    }

    println!("=== Report complete. Evidence > claims. ===");
}

struct Task {
    id: [u8; 16],
    spec: &'static str,
    plan: &'static str,
    diff: &'static str,
    test_log: &'static str,
    outcome: TaskOutcome,
    tools: Vec<(&'static str, u32, u32, u32)>,
}
