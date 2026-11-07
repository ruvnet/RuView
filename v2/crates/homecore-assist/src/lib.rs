//! HOMECORE-ASSIST — Voice/intent pipeline + ruflo agent bridge.
//!
//! Implements [ADR-133](../../../docs/adr/ADR-133-homecore-assist-ruflo.md):
//! the Assist pipeline that takes a voice utterance through intent
//! recognition, intent handling, and response synthesis.
//!
//! ## Module layout
//!
//! - [`intent`] — `IntentName`, `Intent`, `IntentResponse`, `Card`
//! - [`recognizer`] — `IntentRecognizer` trait + `RegexIntentRecognizer`
//! - [`semantic_recognizer`] — `SemanticIntentRecognizer`: real embedding +
//!   ruvector-core HNSW search over enrolled intent exemplars (`semantic` feature)
//! - [`embedding`] — deterministic feature-hash text embedding (`semantic` feature)
//! - [`handler`] — `IntentHandler` trait + 5 built-in HA-mirroring handlers
//! - [`runner`] — `RufloRunner` trait + `LocalRunner` (real recognizer-backed
//!   resolution) + honest `NoopRunner`
//! - [`pipeline`] — `AssistPipeline`: wires recognizer → handler → response
//!
//! ## Implemented capability
//!
//! - Regex-based intent recognition (HA classic intent matching).
//! - Semantic intent recognition: utterance embedding + HNSW nearest-neighbour
//!   match against enrolled exemplars, with a configurable similarity threshold
//!   and regex fallback below it.
//! - Built-in handlers: `HassTurnOn`, `HassTurnOff`, `HassLightSet`,
//!   `HassNevermind`, `HassCancelAll`.
//! - `LocalRunner`: resolves intents locally and returns a real `RufloResponse`
//!   with no external process. `NoopRunner` is an explicit, honest no-op (typed
//!   `NotStarted` before spawn; explicit empty-response after).
//!
//! ## Data-gated / future
//!
//! - A live `node ruflo-agent.js` LLM subprocess runner (Windows-safe teardown
//!   per ADR-133 §Q3) is gated on that script existing; `LocalRunner` is the
//!   honest path until it ships.
//! - STT/TTS bridge and satellite protocol (P3).

pub mod audio;
pub mod handler;
pub mod intent;
pub mod minimax_tts;
pub mod pipeline;
pub mod recognizer;
pub mod runner;
pub mod satellite;
pub mod semantic_recognizer;
pub mod speech;
pub mod voice;

/// Deterministic text embedding used by [`semantic_recognizer::SemanticIntentRecognizer`].
#[cfg(feature = "semantic")]
pub mod embedding;

pub use audio::{AudioChunk, AudioCodec, AudioError, AudioFormat};
pub use handler::{
    HandlerError, HassCancelAll, HassLightSet, HassNevermind, HassTurnOff, HassTurnOn,
    IntentHandler,
};
pub use intent::{Card, Intent, IntentName, IntentResponse};
pub use minimax_tts::{
    MiniMaxAudioOutputFormat, MiniMaxPronunciationDict, MiniMaxRegion, MiniMaxTts,
    MiniMaxTtsConfig, MINIMAX_TTS_AUDIO_FORMATS, MINIMAX_TTS_DEFAULT_MODEL, MINIMAX_TTS_DOCS_URLS,
    MINIMAX_TTS_ENDPOINTS, MINIMAX_TTS_INLINE_OUTPUT_FORMAT, MINIMAX_TTS_MODELS,
    MINIMAX_TTS_REQUEST_FIELDS, MINIMAX_TTS_REQUIRED_FIELDS, MINIMAX_TTS_RESPONSE_FIELDS,
};
pub use pipeline::AssistPipeline;
pub use recognizer::{
    IntentRecognizer, RecognizerError, RegexIntentRecognizer, MAX_UTTERANCE_BYTES,
};
pub use runner::{
    AssistError, LocalRunner, NoopRunner, RufloResponse, RufloRunner, RufloRunnerOpts,
};
pub use satellite::{
    SatelliteClientMessage, SatelliteError, SatelliteServerMessage, SatelliteSession,
    SatelliteState, SATELLITE_PROTOCOL_VERSION,
};
pub use semantic_recognizer::{SemanticIntentRecognizer, DEFAULT_SIMILARITY_THRESHOLD};
pub use speech::{
    DisabledStt, DisabledTts, SpeechError, SpeechToText, SynthesizedSpeech, TextToSpeech,
    Transcript,
};
pub use voice::{VoiceError, VoicePipeline, VoiceResponse, DEFAULT_VOICE_TIMEOUT};
