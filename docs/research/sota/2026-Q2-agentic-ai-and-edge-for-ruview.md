# Agentic-AI Breakthroughs & Related SOTA — Applicability to RuView (2026-Q2)

> **Status:** Research note — non-binding survey. Nothing here is an accepted decision.
> **Date:** 2026-05-11 · **Author:** research pass (Claude Code) · **Scope owner:** ruv
> **Companion docs:** `docs/research/sota/2026-Q2-rf-sensing-and-edge-rust.md`,
> `docs/research/sota-surveys/wifi-sensing-ruvector-sota-2026.md`,
> `docs/research/sota-surveys/ruview-multistatic-fidelity-sota-2026.md`
> **Maps onto ADRs:** 015, 016, 017, 024, 027, 028, 029–032, 039, 040, 069, 081, 084–086, 095, 096

---

## 0. TL;DR — the eight findings that matter for RuView

| # | Breakthrough / SOTA result | Why it matters here | Lands against | Horizon |
|---|---|---|---|---|
| 1 | **WiFi-sensing foundation models scale with *data*, not capacity** — a 14-dataset / 1.3 M-CSI-sample MAE study (arXiv 2511.18792) shows log-linear cross-domain gains from pre-training breadth; larger models barely help. AM-FM (2026) pushes this to ~9.2 M samples / 20 device types. | This is the answer to **MERIDIAN (ADR-027)**: the path to cross-room generalisation is a CSI MAE pre-trained on heterogeneous capture, then a tiny task head — *not* a bigger pose net. RuView already has the data-collection plumbing (`scripts/collect-training-data.py`, overnight recordings, MM-Fi/Wi-Pose under ADR-015). | ADR-015, ADR-016, ADR-027 | Medium |
| 2 | **MAE on amplitude *and* phase** — CIG-MAE (arXiv 2512.04723) reconstructs both with a symmetric dual-stream encoder + information-guided masking; phase reconstruction is now SOTA-competitive. | RuView throws away most phase information after `phase_align.rs` / `coherence.rs`. A dual-stream amplitude+phase MAE pre-text task fits the existing `wifi-densepose-train` graph and the AETHER contrastive head (ADR-024). | ADR-016, ADR-024 | Medium |
| 3 | **Source-free unsupervised domain adaptation works for Wi-Fi** — MU-SHOT-Fi (arXiv 2605.01369), Wi-SFDAGR — adapt a deployed model to a new environment with *no source data and no labels*, just the target stream. | This is exactly the **recalibration gate** decision in `ruvsense/coherence_gate.rs` (`Recalibrate`). Today that gate only flags drift; SFDA gives it something to *do* — adapt the head online, on-device, without phoning home. Pairs naturally with SONA / MicroLoRA + EWC++ already in the stack. | ADR-027, ADR-081, ADR-095/096 | Medium |
| 4 | **The "agent harness on the MCU" pattern** — ESP-Claw (Espressif) and the broader hybrid edge/cloud consensus: heavy reasoning stays in the cloud, but the *loop* (sense → decide → act, plus skills/memory/routing) runs on the microcontroller. ESP32-P4 (dual RISC-V 400 MHz, 768 KB SRAM, 32 MB PSRAM); ESP32-S3 vector ISA accelerates NN kernels. | RuView's ESP32 firmware is already a tiny agent: `adaptive_controller.c` (ADR-081) is the decide loop, the WASM tier (ADR-040) is the skill sandbox, `temporal_task.c` (ADR-095/096) is the on-device model, `edge_processing.c` is sensor fusion. Worth re-framing the firmware explicitly as a *constrained autonomous agent* and stealing patterns (skill registry, memory budget, watchdog-as-supervisor). ESP32-P4 is a credible next hardware tier (vs. the S3's ~200 KB free heap). | ADR-040, ADR-081, ADR-095/096, ADR-028 | Short→Medium |
| 5 | **Agent memory has matured into a discipline** — hierarchical episodic/semantic/procedural memory, the LOCOMO benchmark, the ICLR-2026 *MemAgents* workshop, "Memory in the Age of AI Agents" survey, mem0's "State of Agent Memory 2026". | RuView/ruflo already runs a ReasoningBank-style loop (HNSW-indexed trajectories, verdict→distil→consolidate). The new framing adds: (a) *procedural* memory as a first-class type — the **fix-marker witness guard** (`scripts/fix-markers.json`, merged 2026-05-11) is a primitive instance of this; (b) typed-memory eval (LOCOMO-style) for the dev-loop memory. | ADR-016 (memory side), repo CI | Short |
| 6 | **Long-horizon agents + continual learning are converging into "the system, not the model, is the unit of progress"** — METR's task-duration-doubling (~1 h tasks early-2025 → multi-hour by late-2026); Q2-2026 long-horizon agents from the frontier labs; continual-learning + world-models maturing together. | Two reads for RuView: (a) the *dev* workflow — multi-PR, multi-day swarm work is now tractable; the witness/fix-marker guard is the kind of "system memory" that makes long-horizon dev safe (don't silently revert). (b) the *product* — a RuView deployment that *learns its room over weeks* (longitudinal biomechanics drift in `ruvsense/longitudinal.rs`, the persistent field model ADR-030) is the same "world model that adapts" story, just for RF. | ADR-030, ADR-027, repo workflow | Medium |
| 7 | **Streaming / drift-tolerant vector quantisation** — CoDEQ (arXiv 2512.18335, in ruvector): frozen kd-tree + live leaf centroids via Welford → O(1) updates, no k-means retrain, 7.5× faster build than PQ, ~7 % standalone recall@10 (coarse pre-filter only). RaBitQ / Extended-RaBitQ (arXiv 2405.12497 + follow-ups) remains the high-recall 1-bit workhorse. | RuView's vector tier is RaBitQ + HNSW (ADR-084/085, ADR-016). CoDEQ is **not** a replacement (recall too low) and its "no k-means" advantage is over PQ, which RuView doesn't use. It's a potential *new tier* only if (a) on-ESP32 adaptive quant becomes a goal, or (b) HNSW build cost shows up in a profile as the mesh scales. Deferred — see Part C. | ADR-016, ADR-084/085 | Defer |
| 8 | **Agentic verification / "show your work" is now an explicit research theme** (eval harnesses, trajectory judging, reproducibility gates). | RuView is ahead of the curve here: the ADR-028 deterministic-pipeline proof (`archive/v1/data/proof/verify.py` → SHA-256), the release witness bundle, and the new fix-marker regression guard *are* the "verifiable agent output" pattern applied to a codebase. Worth writing this up as a reference pattern (it's reusable beyond RuView). | ADR-028, repo CI | Done / extend |

**One-line recommendation:** the highest-leverage move is **#1 + #2 + #3 together** — a heterogeneous-CSI masked-autoencoder pre-train (amplitude+phase) plus a source-free online-adaptation hook on the recalibration gate. That's the credible path to closing MERIDIAN, and every piece of plumbing it needs already exists in the repo. Everything else is incremental or already in flight.

---

## 1. Method & scope

This note surveys two adjacent literatures as of 2026-Q2 and filters hard for RuView relevance:

- **Agentic AI** — long-horizon agents, agent memory architectures, self-improving / continual-learning agents, multi-agent coordination, on-device ("edge") agents, retrieval & memory compression, agentic evaluation/verification.
- **RF / WiFi sensing** — CSI foundation models & self-supervised pre-training, domain adaptation, the DensePose-from-WiFi lineage, multistatic / distributed sensing, mmWave + WiFi fusion, adversarial robustness & privacy, through-wall / stand-off radar.

Selection bias is deliberate: a result is in only if there's a concrete hook into an existing RuView crate, ADR, or workflow. Pure-LLM-application work (browser agents, code agents, RAG-over-docs) is out except where it has changed how *this project's* dev loop or firmware should be structured. References (Part E) carry arXiv IDs / DOIs where available.

---

## PART A — Agentic-AI breakthroughs (the relevant slice)

### A1. Long-horizon agents and "the system is the unit of progress"

The headline 2026 shift isn't a model — it's that agents can now hold a goal across hours and many steps. METR's measurement (AI task duration doubling roughly every seven months) crossed from ~1-hour tasks in early 2025 toward multi-hour workstreams by late 2026, and the frontier labs are explicitly targeting "long-horizon agents" for H1-2026. Commentary across the field (adaline labs' "Beyond Transformers", the *Architecture of Agency* guide, MLM's "7 trends") converges on: when continual learning + long-horizon planning + world models mature *simultaneously*, the unit of engineering stops being "a model" and becomes "a system" — harness + memory + tools + verification.

**RuView implications.** Two, on two different timescales:

1. *Dev workflow* (already realised this session). Multi-PR, multi-day swarm work is tractable, and the failure mode is *forgetting / silently reverting* a hard-won fix. The fix-marker witness guard (`scripts/check_fix_markers.py` + `scripts/fix-markers.json`, the `Fix-Marker Regression Guard` workflow, merged in PR #526) is exactly the "system memory that survives the agent" primitive that makes long-horizon dev safe. It's a procedural-memory artifact (see A5) and a verification artifact (see A7) at once. **Action:** keep growing the manifest; treat it as the project's "don't regress" ledger.
2. *Product* (medium horizon). A RuView install that *learns a room over weeks* — longitudinal biomechanics drift (`ruvsense/longitudinal.rs`, Welford stats), the persistent field-model eigenstructure (ADR-030, `ruvsense/field_model.rs`), cross-room fingerprinting (`ruvsense/cross_room.rs`) — is the same "world model that adapts" pattern, in the RF domain. The agentic literature's framing (episodic→semantic→procedural consolidation) is a useful lens for organising what the deployed node should remember about its environment vs. discard.

### A2. Agent memory has become a discipline

2026 is the year "agent memory" stopped being an implementation detail. Signals: the **LOCOMO** benchmark for long-term conversational memory (the first apples-to-apples comparison of memory architectures); the **ICLR-2026 MemAgents workshop**; the *"Memory in the Age of AI Agents: A Survey"* paper list; mem0's "State of AI Agent Memory 2026". The settled taxonomy: **episodic** (what happened), **semantic** (distilled facts), **procedural** (how to do things / skills) — with consolidation passes promoting episodic → semantic → procedural, hierarchical stores, and (newer) multi-agent *shared* memory.

**RuView/ruflo already runs a ReasoningBank-style loop** — HNSW-indexed trajectory store, verdict-judge → distil → consolidate, experience replay (this is in the `.claude-flow/` coordination layer and the `reasoningbank-*` skills). What the 2026 framing adds:

- **Procedural memory as a first-class artifact.** The fix-marker manifest is procedural memory for the *codebase*: "the way we do CSI capture is `WIFI_PS_NONE` before promiscuous (#521); the way we handle sibling UDP magics is `ruview_sibling_packet_name` (#517); …". It's checked-in, diffable, CI-enforced. That's exactly what the literature recommends and almost nobody does. **Action:** generalise the pattern — a `docs/research/`-adjacent "decisions ledger" that links ADRs ↔ code markers ↔ tests is a natural extension.
- **Typed-memory evaluation.** A LOCOMO-style harness for the *dev-loop* memory (does recall surface the right ADR/pattern for a given task?) would be cheap and would catch memory rot. Low priority but easy.
- **Sensor-side memory.** The deployed ESP32 node has its own (tiny) episodic/semantic split: `edge_processing.c` ring buffer (episodic), the rolling vitals/presence stats (semantic), the NVS config + adaptive-controller policy (procedural). Worth being explicit about the budget (the S3 has ~200 KB free heap) and what gets consolidated vs. dropped.

### A3. Self-improving / continual-learning agents

The "self-improving" thread: agents that analyse past outcomes and evolve their strategies (this is the verdict→distil loop again, plus online weight updates). The hard problem is doing it without catastrophic forgetting. RuView is *already living in this space*: **SONA** (self-optimising neural architecture) + **MicroLoRA** online adaptation + **EWC++** for forgetting-resistance is named in the project's own positioning, and ADR-024 (AETHER contrastive CSI embedding) + ADR-095/096 (on-ESP32 temporal head with sparse GQA) are the substrate.

**What's new and worth pulling in:**

- **Source-free domain adaptation as the *update rule*** (see B2) — MU-SHOT-Fi / Wi-SFDAGR show you can adapt to a new environment with target stream only. Wire that as the action behind `coherence_gate.rs::Recalibrate`: when the coherence z-score gate says "this room has drifted", run an SFDA pass on the temporal head (MicroLoRA delta, EWC++ regulariser, bounded steps) instead of just degrading to `PredictOnly`.
- **Bounded, auditable online learning.** The fix-marker / witness culture should extend to learned weights: every on-device adaptation event should be logged (the firmware already has the witness-chain primitives from the Cognitum/`brain` work and ADR-028) so "the model in this room is now divergent from the shipped checkpoint" is *observable*, not silent — the same lesson as #505 (mislabelled firmware), one layer up.

### A4. Multi-agent coordination at the edge — RuView's mesh *is* a swarm

A 4–6 node ESP32 RuView deployment is, structurally, a multi-agent system: each node senses, runs an edge-tier pipeline, makes local decisions (channel hop, role, send-rate via `adaptive_controller.c`), and they fuse via TDM + multistatic attention. The agentic-systems literature (the arXiv 2601.12560 architectures/taxonomies paper; Byzantine-consensus and market-based task-allocation work) maps cleanly:

- **Role assignment / task allocation** — which node is the coordinator, which compute the per-cluster π hop (ADR-083), which go `PredictOnly` when degraded — is a market/auction problem. RuView does this ad hoc today; the literature has cleaner protocols.
- **Byzantine robustness** — ADR-032 (multistatic mesh security hardening) and `ruvsense/adversarial.rs` (physically-impossible-signal detection, multi-link consistency) are RuView's answer to "a node lies / is spoofed". The 2026 framing: treat it as Byzantine-fault-tolerant sensor fusion with an explicit fault model, not just heuristics.
- **Shared memory across the swarm** — the multi-agent-shared-memory thread suggests the nodes should converge on a shared *field model* (ADR-030) the way agents converge on shared semantic memory: CRDT-style, eventually consistent, with the coordinator as a soft leader (raft-ish). Some of this is sketched in `radio_ops.rs` (mesh header, node status, anomaly alerts).

**Action:** a short ADR re-framing the firmware mesh as a BFT sensor swarm with explicit role-auction + shared-field-model-as-CRDT would unify ADR-029/030/032/081/083 and give the implementation a literature to lean on. Low urgency, high coherence value.

### A5. The "agent harness on the MCU" pattern (most directly actionable)

Espressif's **ESP-Claw** crystallised something RuView half-built already: keep the LLM in the cloud, but run the *whole agent loop* — skills, tools, memory, routing, the sense→decide→act cycle — on the microcontroller. The hardware backs it: **ESP32-P4** (dual RISC-V @ 400 MHz, ~768 KB internal SRAM, 32 MB PSRAM, HW H.264) is Espressif's AI-focused part; the **ESP32-S3** vector ISA already accelerates NN kernels (which is why ADR-095/096's on-device temporal head is viable at all). The broader 2026 consensus is *hybrid*: MCU runs lightweight models + rule-based agents for real-time decisions, offload heavy reasoning only when needed.

**RuView's firmware is already a constrained agent — name it as one:**

| Agent component (ESP-Claw-ish) | RuView firmware equivalent |
|---|---|
| Decide loop | `adaptive_controller.c` (ADR-081) — fast/med/slow ticks, channel/role/send-rate policy |
| Skill sandbox | WASM tier (ADR-040) — uploadable `.wasm` modules, `on_timer()`, the upload/list/start/stop endpoints |
| On-device model | `temporal_task.c` (ADR-095/096) — sparse-GQA temporal head, emits `0xC5110007` classifications |
| Sensor fusion / perception | `edge_processing.c` — vitals, presence, fall detection, feature vectors; `csi_collector.c` — capture + the `WIFI_PS_NONE` fix |
| Memory | NVS config (procedural), ring buffer (episodic), rolling stats (semantic), witness chain (audit) |
| Supervisor / watchdog | `task_wdt` + the `EDGE_BATCH_LIMIT` yield discipline (#266/#321) — "kill the agent if it starves the idle task" |
| Telemetry / trajectory | UDP packet stream (`0xC5110001`–`0xC5110007`), boot log, OTA status |

**Actions worth doing:**
1. **An ADR that explicitly models the firmware as a tiered autonomous agent** (Tier-0 rules → Tier-1 WASM skills → Tier-2 on-device temporal head → Tier-3 cloud), with a stated memory budget and a "supervisor" contract. This mostly *documents and unifies* ADR-040/081/086/095/096 — but the framing buys clarity and a literature.
2. **Track ESP32-P4 as a real hardware tier.** The S3's ~200 KB free heap is the binding constraint on the temporal head and the WASM tier; the P4's 32 MB PSRAM changes that calculus. Worth a feasibility note (and it's a natural place for a bigger MAE-derived head).
3. **Steal ESP-Claw's skill-registry ergonomics** for the WASM tier (ADR-040) — versioned skills, declared capabilities, a deny-by-default policy (this echoes the `brain_sdk_allow`/`deny` pattern already in the `logi-brain` MCP surface).

### A6. Retrieval & memory compression

The fast-moving sub-area: 1-bit / few-bit vector quantisation with theoretical error bounds. **RaBitQ** (arXiv 2405.12497) and **Extended-RaBitQ** are the high-recall workhorses — and RuView already runs them (ADR-084 RaBitQ similarity sensor, ADR-085 pipeline expansion, plus the ruvector RaBitQ binding). **CoDEQ** (arXiv 2512.18335, in ruvector) is the new streaming/drift-tolerant entrant: frozen kd-tree + Welford-updated leaf centroids, O(1) updates, 7.5× faster build than PQ, but ~7 % standalone recall@10 — a coarse pre-filter, never a sole index. **Graph RAG / hybrid (sparse+dense) retrieval** with MMR diversity reranking is the other settled pattern (and `ruflo-rag-memory` already implements it for the dev loop).

**RuView read (see Part C for the verdict):** RaBitQ + HNSW (current) is the right stack for high-recall CSI/pose matching. CoDEQ is *deferred* — its "no k-means retrain" edge is over PQ (which RuView doesn't use), and there's no measured bottleneck it relieves. It becomes interesting only on the ESP32 (where a tiny, streaming, retrain-free quantiser might be the *only* thing that fits) — i.e. it's a possible ingredient of the ADR-095/096 line, not of the server-side index.

### A7. Agentic verification / "show your work"

A genuine 2026 research theme: eval harnesses, trajectory judging, reproducibility gates, "the agent must produce a verifiable artifact." RuView is, unusually, *ahead* here:

- **ADR-028 deterministic-pipeline proof** — `archive/v1/data/proof/verify.py` feeds a seeded reference signal through the production pipeline and SHA-256-hashes the output; `expected_features.sha256` pins it; `verify-pipeline.yml` re-runs it on every PR (twice, for determinism). This is a "tamper-evident agent output" by construction.
- **Release witness bundle** — `scripts/generate-witness-bundle.sh` → a recipient-verifiable `VERIFY.sh` packet (witness log + proof + test results + firmware hashes + crate versions).
- **Fix-marker regression guard** (new, merged PR #526) — `scripts/fix-markers.json` + `scripts/check_fix_markers.py` + the `Fix-Marker Regression Guard` workflow: every shipped fix asserts its own continued presence; reverting one fails CI; intentional removal forces a manifest diff (= the audit trail).
- **`firmware-ci.yml` version-guard** (new) — a release tag can't ship a binary whose `version.txt` doesn't match the tag (the #505 lesson, automated).

**Action:** write this up as a reusable pattern (it generalises well beyond RuView — it's basically "ReasoningBank verdicts, but for a repo"). A `docs/` note or a public gist; possibly an ADR ("verification artifacts as a project contract"). The `ruflo-core:witness` plugin is the cross-project version of the same idea — worth a cross-reference.

---

## PART B — RF / WiFi-sensing SOTA (related research)

### B1. WiFi-sensing foundation models & self-supervised CSI — *the big one*

The dominant 2025→2026 result: **masked autoencoding (MAE) pre-training on large, heterogeneous CSI pools beats supervised baselines on cross-domain tasks, and the bottleneck is data breadth, not model size.**

- *"Scale What Counts, Mask What Matters: Evaluating Foundation Models for Zero-Shot Cross-Domain Wi-Fi Sensing"* (arXiv 2511.18792) — pre-trains/evaluates across **14 datasets, >1.3 M CSI samples, 4 device types, 2.4/5/6 GHz**; finds **log-linear** cross-domain gains with pre-training data (+2.2 % to +15.7 % over supervised), **marginal** gains from bigger models. Tasks: activity, gesture, user-ID.
- **AM-FM** (2026) — billed as the first true WiFi foundation model: **~9.2 M samples, ~20 device types**.
- *"A Tutorial-cum-Survey on Self-Supervised Learning for Wi-Fi Sensing"* (arXiv 2506.12052) and the ACM TOSN evaluation (DOI 10.1145/3715130) — MAE is the consistently strong SSL choice for CSI.
- **CIG-MAE** (arXiv 2512.04723) — dual-stream MAE reconstructing **both amplitude and phase**, with information-guided masking (mask the high-info regions). Phase reconstruction is now competitive — historically the hard part.
- **CIR–CSI consistency** (arXiv 2502.11965), **WiFo-CF** (arXiv 2508.04068) — channel/CSI-feedback foundation models from the comms side; relevant as architecture priors and for the multi-link / MIMO framing.

**RuView mapping.** This *is* the MERIDIAN program (ADR-027) — and RuView already has the pieces:
- Data plumbing: `scripts/collect-training-data.py`, `scripts/collect-ground-truth.py`, overnight CSI recordings, MM-Fi + Wi-Pose ingestion (ADR-015), the `data/recordings/` corpus.
- Training graph: `wifi-densepose-train` (ADR-016, ruvector-integrated) — a place to bolt an MAE pre-text head on.
- Embedding: AETHER contrastive head (ADR-024) — natural fine-tune target after MAE pre-train.
- Compression: `CompressedCsiBuffer` (`dataset.rs`, ruvector-temporal-tensor) — already streams CSI history; a sensible substrate for masked-token pre-training.

**Concrete plan (this is the recommendation):** ADR-027 should become "**heterogeneous-CSI MAE pre-train (amplitude+phase, CIG-MAE-style) → small task head**", with the explicit thesis *data breadth > pose-net capacity*. Phase 1: pool every CSI source RuView can reach (own recordings, MM-Fi, Wi-Pose, public CSI datasets, multi-band virtual subcarriers from `multiband.rs`) and run an MAE pre-train. Phase 2: fine-tune the 17-keypoint head + AETHER embedding on top. Phase 3: ship the encoder; the per-room work becomes adaptation (B2), not retraining.

### B2. Source-free / unsupervised domain adaptation

- **MU-SHOT-Fi** (arXiv 2605.01369) — self-supervised *multi-user* Wi-Fi sensing with **source-free** unsupervised domain adaptation: adapt with target stream only, no source data, no labels.
- **Wi-SFDAGR** — WiFi cross-domain gesture recognition via source-free domain adaptation (IEEE).
- *Self-supervised WiFi-based identity recognition in multi-user smart environments* (PMC12115556) — relevant for AETHER re-ID across rooms.

**RuView mapping.** This is the *missing action* behind `ruvsense/coherence_gate.rs::Recalibrate` and the recalibration recommendations in `coherence.rs` (`RECOMMEND_RECAL` quality flag, already on the wire in `rv_feature_state.h`). Today the gate detects environment drift and degrades; SFDA lets it *fix* itself: a bounded MicroLoRA-delta adaptation pass on the temporal head, EWC++-regularised, triggered by the coherence z-score, logged via the witness chain. Multi-user SFDA (MU-SHOT-Fi) is directly relevant because RuView's whole point is multi-person (the `DynamicPersonMatcher`, the COCO-17 multi-track output).

### B3. The DensePose-from-WiFi lineage — where RuView sits

Origin: **CMU's "DensePose From WiFi"** (Geng et al., arXiv 2301.00250, building on the 2022 RI thesis CMU-RI-TR-22-59) — UV-coordinate dense pose from CSI using 3×3 MIMO commercial NICs (Intel 5300 / Atheros). The honest gap (well-documented in RuView's own issues #506/#509): that work relies on rich multi-antenna spatial resolution; ESP32 is 1×1 SISO. RuView's bet is to recover spatial diversity *across nodes* (4–6 ESP32, TDM, multistatic attention-weighted fusion, 168 virtual subcarriers via 3 channels × 56) rather than within one rich NIC — plus a Rust pipeline at sub-50 ms (~800× over the original Python) and SONA on-device adaptation. The foundation-model results (B1) are what make the ESP32 path plausible: a strong pre-trained CSI encoder lowers how much the noisy SISO multistatic input has to carry.

### B4. Multistatic / distributed RF sensing & multi-band fusion

Active area, and RuView's `ruvsense/` is a fairly complete implementation of it: multi-band frame fusion + cross-channel coherence (`multiband.rs`), iterative LO phase-offset estimation (`phase_align.rs`), attention-weighted multistatic fusion with geometric diversity (`multistatic.rs`), RF tomography with an ISTA L1 solver (`tomography.rs`), the persistent room-eigenstructure field model (`field_model.rs`, ADR-030), cross-viewpoint attention with geometric bias and Cramér-Rao / Fisher-information bounds (`ruvector/src/viewpoint/`). The SOTA reading is mostly: the geometry-aware-attention + information-bound framing RuView already uses is the right one; the gaps are (a) a cleaner statistical fault model (→ A4, ADR-032) and (b) tying the field model to the foundation-model encoder (a pre-trained encoder + a per-room eigenstructure prior is a strong combo).

### B5. mmWave + WiFi fusion, vital signs

ADR-063 (mmWave sensor fusion, ESP32-C6 + Seeed MR60BHA2 over UART, 60 GHz FMCW) and ADR-021 (ESP32 CSI-grade vital-sign extraction, the `wifi-densepose-vitals` 4-stage pipeline) put RuView in the multimodal-vitals SOTA. The literature trend: WiFi gives coarse presence/macro-motion + room-scale coverage; 60 GHz FMCW gives precise HR/BR but narrow FOV; fusion (Kalman / attention) beats either. RuView's `mmwave_fusion_bridge.py` + the `0xC5110004` fused-vitals packet are the implementation. Watch: the cardiac/respiration-from-CSI-alone work keeps improving (RuView already does breathing/HR from CSI in `breathing.rs` / `bvp.rs`) — a good MAE pre-train (B1) should help here too since respiration is a periodic-structure problem.

### B6. Adversarial robustness & privacy

`ruvsense/adversarial.rs` (physically-impossible-signal detection, multi-link consistency) + ADR-032 (multistatic mesh security) are RuView's stake. The 2026 framing: (a) **spoofing/jamming** as Byzantine faults in a sensor swarm (→ A4) with a stated adversary model; (b) **privacy** — WiFi sensing is "camera-free" but still biometric (gait, breathing, re-ID embeddings are PII); the project already gestures at this (privacy logs in the ADR-084 RaBitQ-sensor framing), and the broader move is toward on-device-only processing + differential-privacy on any exported embedding. The `aidefence` / PII-detection surface in the ruflo toolchain is the dev-side analogue.

### B7. Through-wall / NLOS, stand-off radar tier

ADR-091 (stand-off radar tier research) and the single-sided through-wall thread (issue #424) sit here. SOTA: through-wall pose/activity is real but resolution-limited; multistatic helps (more look-angles see around the wall differently); the foundation-model encoders (B1) are starting to include NLOS data in their pre-training pools, which is the cleanest path to robustness. Mostly a "watch" item for RuView — the multi-node multistatic architecture is already the right substrate.

---

## PART C — Synthesis: what's actionable for RuView

Prioritised. "Effort" is rough; "horizon" is short (<1 mo) / medium (1–3 mo) / long (>3 mo) / defer.

| Rank | Action | Impact | Effort | Horizon | New ADR? | Notes |
|---|---|---|---|---|---|---|
| **1** | **MERIDIAN ⇒ heterogeneous-CSI MAE pre-train (amplitude+phase, CIG-MAE-style) → small task head.** Pool all reachable CSI (own recordings + MM-Fi + Wi-Pose + public + multi-band virtual subcarriers), MAE pre-train in `wifi-densepose-train`, fine-tune the 17-kpt + AETHER heads on top. | Very high — this is the cross-room story | High | Long | Re-scope ADR-027; possibly fold ADR-016 | All plumbing exists. Thesis: *data breadth > pose-net capacity* (2511.18792). |
| **2** | **Source-free online adaptation as the action behind `coherence_gate.rs::Recalibrate`.** Bounded MicroLoRA-delta + EWC++ pass on the temporal head, triggered by the coherence z-score, logged via the witness chain. | High — turns a "detect" into a "fix" | Medium | Medium | New ADR (sibling to ADR-027/081/095) | MU-SHOT-Fi / Wi-SFDAGR. Multi-user variant matters (RuView is multi-person). |
| **3** | **ADR: "firmware as a tiered autonomous agent."** Document & unify ADR-040/081/086/095/096 under the ESP-Claw-style agent model (Tier-0 rules → Tier-1 WASM skills → Tier-2 on-device temporal head → Tier-3 cloud), with a memory budget and a supervisor contract. | Medium — clarity + a literature to lean on | Low | Short | Yes (mostly documentation) | Cheap; high coherence value. |
| **4** | **Track ESP32-P4 as a hardware tier.** Feasibility note: 32 MB PSRAM vs. the S3's ~200 KB free heap changes what the temporal head / WASM tier / a bigger MAE-derived head can be. | Medium — unblocks the on-device-model ceiling | Low | Short | Note → ADR if pursued | Espressif's AI-focused part; S3 vector ISA stays the floor. |
| **5** | **Write up the verification-artifact pattern** (ADR-028 proof + witness bundle + fix-marker guard + version-guard) as a reusable reference; cross-link `ruflo-core:witness`. Keep growing `fix-markers.json`. | Medium — reuse beyond RuView; protects long-horizon dev | Low | Short | Optional ADR | RuView is ahead of the field here; make it legible. |
| **6** | **ADR: firmware mesh as a BFT sensor swarm** — explicit role-auction + shared-field-model-as-CRDT + a stated adversary model. Unifies ADR-029/030/032/081/083. | Medium — coherence; sets up ADR-032 properly | Medium | Medium | Yes | Lean on the agentic-systems / Byzantine-sensor-fusion literature. |
| **7** | **Phase-aware features end-to-end.** RuView discards most phase after `phase_align.rs`/`coherence.rs`; CIG-MAE shows phase reconstruction is now SOTA-competitive. Carry an amplitude+phase representation into the embedding. | Medium | Medium | Medium | Folds into #1 | Likely just falls out of #1 if the MAE is dual-stream. |
| **8** | **CoDEQ — DEFER, with a stub.** Add a one-paragraph "alternatives considered" to ADR-017: streaming/drift-tolerant coarse quant; no current bottleneck; revisit only if (a) on-ESP32 adaptive quant becomes a goal or (b) HNSW build/rebuild shows up in a profile as the mesh scales. | Low (now) | Trivial | Defer | Stub in ADR-017 | RaBitQ+HNSW is the right stack today; CoDEQ's "no k-means" edge is over PQ, which RuView doesn't use. |

**If you do one thing:** #1. If you do two: #1 + #2 (they're complementary — pre-train for breadth, adapt for the room). #3/#4/#5 are cheap and worth slipstreaming. #6 is housekeeping that pays off when ADR-032 gets serious. #7 is probably free given #1. #8 is "don't lose the idea."

---

## PART D — Watch list / open questions

- **AM-FM and the next WiFi foundation models** — when one ships with permissive weights + a CSI tokeniser RuView can adopt, #1 gets dramatically cheaper. Watch arXiv / Hugging Face.
- **Phase-faithful CSI capture on ESP32** — how much usable phase does the S3 actually deliver under the MGMT-only promiscuous regime (#396)? Worth a measurement; gates how much of CIG-MAE's amplitude+phase advantage RuView can realise on cheap hardware.
- **On-device MAE-derived heads** — is a distilled/quantised MAE encoder small enough for the S3 (or does it need the P4)? Determines whether the foundation model lives only server-side or also on the node.
- **LOCOMO-style eval for the dev-loop memory** — does ReasoningBank recall actually surface the right ADR/pattern? Cheap to measure; would catch memory rot.
- **Byzantine fault model for the mesh** — pin down the adversary (spoofed node? jammed link? compromised firmware? replay?) before ADR-032 implementation, not after.
- **Differential privacy on exported embeddings** — AETHER re-ID embeddings are biometric; if any leave the box (multi-room hand-off, cloud tier), what's the DP budget?
- **CoDEQ revisit trigger** — only if a profile shows HNSW build/rebuild as a bottleneck, or if on-ESP32 adaptive quant becomes a stated goal.
- **ESP-Claw / on-MCU agent frameworks** — track Espressif's releases; the skill-registry / capability-policy ergonomics are directly stealable for the WASM tier (ADR-040).

---

## PART E — References

WiFi / RF sensing:
- DensePose From WiFi — Geng et al., arXiv [2301.00250](https://arxiv.org/abs/2301.00250); CMU RI thesis CMU-RI-TR-22-59.
- Scale What Counts, Mask What Matters: Evaluating Foundation Models for Zero-Shot Cross-Domain Wi-Fi Sensing — arXiv [2511.18792](https://arxiv.org/html/2511.18792).
- CIG-MAE: Cross-Modal Information-Guided Masked Autoencoder for Self-Supervised WiFi Sensing — arXiv [2512.04723](https://arxiv.org/html/2512.04723v1).
- MU-SHOT-Fi: Self-Supervised Multi-User Wi-Fi Sensing with Source-free Unsupervised Domain Adaptation — arXiv [2605.01369](https://arxiv.org/html/2605.01369).
- A Tutorial-cum-Survey on Self-Supervised Learning for Wi-Fi Sensing — arXiv [2506.12052](https://arxiv.org/html/2506.12052).
- Evaluating Self-Supervised Learning for WiFi CSI-Based Human Activity Recognition — ACM TOSN, [10.1145/3715130](https://dl.acm.org/doi/10.1145/3715130).
- A MIMO Wireless Channel Foundation Model via CIR–CSI Consistency — arXiv [2502.11965](https://arxiv.org/html/2502.11965).
- WiFo-CF: Wireless Foundation Model for CSI Feedback — arXiv [2508.04068](https://arxiv.org/pdf/2508.04068).
- Wi-SFDAGR: WiFi-Based Cross-Domain Gesture Recognition via Source-Free Domain Adaptation — IEEE (DOI per IEEE Xplore listing).
- Self-Supervised WiFi-Based Identity Recognition in Multi-User Smart Environments — PMC [PMC12115556](https://pmc.ncbi.nlm.nih.gov/articles/PMC12115556/).
- (project context) RuView issues #68 (MERIDIAN/ADR-027), #506, #509, #424.

Agentic AI / memory / edge:
- Agentic Artificial Intelligence (AI): Architectures, Taxonomies, and Evaluation of LLM Agents — arXiv [2601.12560](https://arxiv.org/html/2601.12560v1).
- MemAgents: Memory for LLM-Based Agentic Systems — ICLR-2026 workshop proposal, OpenReview [U51WxL382H](https://openreview.net/pdf?id=U51WxL382H).
- Memory in the Age of AI Agents: A Survey — paper list: github.com/Shichun-Liu/Agent-Memory-Paper-List.
- 2026 agent papers collection — github.com/VoltAgent/awesome-ai-agent-papers.
- State of AI Agent Memory 2026 — mem0.ai/blog/state-of-ai-agent-memory-2026.
- LOCOMO benchmark (long-term conversational memory) — see the mem0 and agent-memory survey references.
- METR — measuring AI ability to complete long tasks (task-duration doubling).
- ESP-Claw / on-MCU AI agents — Espressif; xda-developers coverage; ESP32-P4 / ESP32-S3 vector ISA datasheets.
- (project) ReasoningBank — the trajectory verdict→distil→consolidate loop RuView/ruflo implements; `reasoningbank-*` skills.

Retrieval / quantisation:
- RaBitQ: Quantizing High-Dimensional Vectors with a Theoretical Error Bound — arXiv [2405.12497](https://arxiv.org/abs/2405.12497); Extended-RaBitQ follow-ups.
- CoDEQ: streaming vector quantisation (frozen kd-tree + Welford leaf centroids) — arXiv 2512.18335; in `ruvector`. Gist: https://gist.github.com/ruvnet/d10fe656bd0fa68b4eb873ad299c6d4e.

RuView internal (for the mapping):
- ADRs: 014, 015, 016, 017, 021, 022, 024, 027, 028, 029, 030, 031, 032, 039, 040, 045, 060, 061, 062, 063, 069, 080–086, 089–096 — see `docs/adr/`.
- Crates: `wifi-densepose-{core,signal,nn,train,mat,hardware,ruvector,api,db,config,wasm,cli,sensing-server,wifiscan,vitals}`, `nvsim` — see project `CLAUDE.md`.
- Modules: `ruvsense/*` (signal crate), `viewpoint/*` (ruvector crate); firmware `main/*.c`.
- Verification artifacts: `archive/v1/data/proof/verify.py`, `scripts/generate-witness-bundle.sh`, `scripts/check_fix_markers.py` + `scripts/fix-markers.json`, `.github/workflows/{verify-pipeline,firmware-ci,fix-regression-guard}.yml`.
- Related surveys in this repo: `docs/research/sota/2026-Q2-rf-sensing-and-edge-rust.md`, `docs/research/sota-surveys/{wifi-sensing-ruvector-sota-2026,ruview-multistatic-fidelity-sota-2026,sota-wifi-sensing-2025}.md`, `docs/research/rf-topological-sensing/*`.

---

*Generated by Claude Code (research pass), 2026-05-11. Treat as input to ADR discussions, not as decisions.*
