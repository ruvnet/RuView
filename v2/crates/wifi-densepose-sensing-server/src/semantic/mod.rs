//! ADR-115 §3.12 — Semantic Automation Primitives (HA-MIND).
//!
//! Raw signals are not the product. Customers want first-class entities
//! like `binary_sensor.bedroom_someone_sleeping`, not a Node-RED flow
//! that thresholds breathing rate at night. This module owns the
//! inference layer that turns the `sensing-server` broadcast (raw
//! `edge_vitals` / `pose_data` / `sensing_update`) into the 10 v1
//! semantic primitives published as HA entities, Matter events, and
//! Apple Home scene triggers.
//!
//! ## Architectural contract
//!
//! - **Server-side inference.** All primitives run inside this process.
//!   Only the inferred *state* (true/false, scalar, event) crosses the
//!   wire. This is what makes `--privacy-mode` compatible with most
//!   semantic primitives — biometric *values* can be stripped at the
//!   integration boundary while the inferred *states* still publish.
//!   Primitives whose inputs include vital signs (sleeping, distress) are
//!   suppressed with them (#2165).
//! - **One source of truth.** Each primitive's FSM lives in one file
//!   alongside its tests. The `SemanticBus` aggregates output and
//!   broadcasts to MQTT + Matter consumers. Adding a new primitive is
//!   one file change — no new MQTT discovery schema, no new Matter
//!   cluster.
//! - **Explainability.** Every state change carries a `reason`
//!   payload so HA users can debug *why* a primitive fired.
//! - **Hysteresis everywhere.** Each primitive has explicit enter /
//!   exit thresholds + minimum dwell time so a single noisy frame
//!   never toggles state. Refractory periods prevent alert spam.
//! - **Warmup suppression.** No primitive fires during the first 60 s
//!   after start (per §3.12.4 — sensors are still settling).
//!
//! ## Primitives (v1)
//!
//! | Primitive               | Module                | Output           |
//! |-------------------------|-----------------------|------------------|
//! | someone_sleeping        | [`sleeping`]          | binary_sensor    |
//! | possible_distress       | [`distress`]          | binary_sensor + event |
//! | room_active             | [`room_active`]       | binary_sensor    |
//! | elderly_inactivity_…    | [`elderly_anomaly`]   | binary_sensor + event |
//! | meeting_in_progress     | [`meeting`]           | binary_sensor    |
//! | bathroom_occupied       | [`bathroom`]          | binary_sensor    |
//! | fall_risk_elevated      | [`fall_risk`]         | sensor (0-100)   |
//! | bed_exit                | [`bed_exit`]          | event            |
//! | no_movement             | [`no_movement`]       | binary_sensor    |
//! | multi_room_transition   | [`multi_room`]        | event            |
//!
//! Each module exports a struct implementing [`Primitive`] and a `new`
//! constructor that takes a [`PrimitiveConfig`].

mod bathroom;
mod bed_exit;
mod bus;
mod common;
mod distress;
mod elderly_anomaly;
mod fall_risk;
mod meeting;
mod multi_room;
mod no_movement;
mod room_active;
mod sleeping;

// ADR-140: auditable semantic-state record + Ruflo multi-signal agent bridge.
pub mod record;

pub use bus::{SemanticBus, SemanticEvent, SemanticKind};
pub use common::{PrimitiveConfig, PrimitiveState, RawSnapshot, Reason};
pub use record::{
    AgentRoute, MultiSignalRule, PrivacyAction, RecordContext, SemanticStateRecord, route_all,
};
