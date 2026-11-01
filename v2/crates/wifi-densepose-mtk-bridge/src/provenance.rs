//! Gate on where a replay input actually came from.
//!
//! `mediatek:physical-unvalidated` must be reachable only from a real device.
//! A file on disk cannot prove it came from silicon, so the bridge refuses to
//! guess: replaying anything requires the operator either to declare the input
//! synthetic (`--synthetic`) or to attest to the hardware it came off
//! (`--captured-on <model>/<firmware>`).
//!
//! An input can also declare itself synthetic, and that declaration wins over
//! any attestation. Three independent signals are honoured, any one of which is
//! enough:
//!
//! 1. A sidecar `<file>.provenance.json` holding `{"synthetic": true, ...}`.
//! 2. A `.synthetic.` infix in the file name, so a lost sidecar cannot silently
//!    downgrade a fixture's provenance.
//! 3. A capture header whose `synthetic` field is true, which is how a recording
//!    made during a `--synthetic` run stays synthetic on replay.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::BridgeError;

/// Marker written next to a file that is not a hardware capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSidecar {
    pub synthetic: bool,
    /// Free text naming what produced the file.
    #[serde(default)]
    pub generator: String,
}

/// The `.synthetic.` file-name infix that marks a non-hardware input.
pub const SYNTHETIC_INFIX: &str = ".synthetic.";

/// What the input itself claims about its own origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputProvenance {
    pub synthetic: bool,
    pub generator: Option<String>,
    /// Which signal decided this, for the startup log.
    pub evidence: &'static str,
}

impl InputProvenance {
    pub fn undeclared() -> Self {
        Self {
            synthetic: false,
            generator: None,
            evidence: "no declaration",
        }
    }
}

/// Path of the sidecar for an input file.
pub fn sidecar_path(input: &Path) -> PathBuf {
    let mut name = input.file_name().unwrap_or_default().to_os_string();
    name.push(".provenance.json");
    input.with_file_name(name)
}

/// Read what the file itself declares, before its contents are parsed.
pub fn resolve_path_provenance(input: &Path) -> Result<InputProvenance, BridgeError> {
    let sidecar = sidecar_path(input);
    if sidecar.exists() {
        let text = std::fs::read_to_string(&sidecar).map_err(BridgeError::Io)?;
        let parsed: ProvenanceSidecar = serde_json::from_str(&text).map_err(BridgeError::Json)?;
        return Ok(InputProvenance {
            synthetic: parsed.synthetic,
            generator: Some(parsed.generator).filter(|g| !g.is_empty()),
            evidence: "sidecar .provenance.json",
        });
    }
    if file_name_declares_synthetic(input) {
        return Ok(InputProvenance {
            synthetic: true,
            generator: None,
            evidence: "`.synthetic.` in the file name",
        });
    }
    Ok(InputProvenance::undeclared())
}

fn file_name_declares_synthetic(input: &Path) -> bool {
    input
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(SYNTHETIC_INFIX))
}

/// Fold a capture header's own `synthetic` field into the path-level result.
/// Synthetic is sticky: nothing can clear it.
pub fn merge_capture_header(base: InputProvenance, header_synthetic: bool) -> InputProvenance {
    if header_synthetic && !base.synthetic {
        return InputProvenance {
            synthetic: true,
            generator: base.generator,
            evidence: "capture header",
        };
    }
    base
}

/// The decision a replay runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayAuthorization {
    /// Frames are flagged SYNTHETIC and the server labels them simulated.
    Synthetic { reason: String },
    /// Operator attested to real hardware; frames stay physical-unvalidated.
    Attested { captured_on: String },
}

/// Decide whether this replay may run, and under which provenance.
pub fn authorize_replay(
    provenance: &InputProvenance,
    synthetic_flag: bool,
    captured_on: Option<&str>,
) -> Result<ReplayAuthorization, BridgeError> {
    if provenance.synthetic && !synthetic_flag {
        return Err(BridgeError::SyntheticInputNeedsSyntheticFlag {
            evidence: provenance.evidence,
            generator: provenance.generator.clone(),
        });
    }
    if synthetic_flag {
        let reason = if provenance.synthetic {
            format!(
                "--synthetic, input declares itself synthetic via {}",
                provenance.evidence
            )
        } else {
            "--synthetic".to_string()
        };
        return Ok(ReplayAuthorization::Synthetic { reason });
    }
    match captured_on {
        Some(text) => {
            validate_attestation(text)?;
            Ok(ReplayAuthorization::Attested {
                captured_on: text.to_string(),
            })
        }
        None => Err(BridgeError::ReplayNeedsProvenance),
    }
}

/// `--captured-on` must name both a model and a firmware, `<model>/<firmware>`.
fn validate_attestation(text: &str) -> Result<(), BridgeError> {
    let mut parts = text.splitn(2, '/');
    let model = parts.next().unwrap_or("").trim();
    let firmware = parts.next().unwrap_or("").trim();
    if model.is_empty() || firmware.is_empty() {
        return Err(BridgeError::BadAttestation(text.to_string()));
    }
    Ok(())
}

impl ReplayAuthorization {
    pub fn is_synthetic(&self) -> bool {
        matches!(self, Self::Synthetic { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_input() -> InputProvenance {
        InputProvenance {
            synthetic: true,
            generator: Some("test".to_string()),
            evidence: "sidecar .provenance.json",
        }
    }

    #[test]
    fn a_synthetic_input_cannot_be_replayed_without_the_flag() {
        assert!(matches!(
            authorize_replay(&synthetic_input(), false, None),
            Err(BridgeError::SyntheticInputNeedsSyntheticFlag { .. })
        ));
    }

    #[test]
    fn an_attestation_cannot_override_a_synthetic_declaration() {
        assert!(matches!(
            authorize_replay(&synthetic_input(), false, Some("WN586X3/24.10.8")),
            Err(BridgeError::SyntheticInputNeedsSyntheticFlag { .. })
        ));
    }

    #[test]
    fn a_synthetic_input_replays_with_the_flag() {
        let auth = authorize_replay(&synthetic_input(), true, None).unwrap();
        assert!(auth.is_synthetic());
    }

    #[test]
    fn an_undeclared_input_still_needs_one_of_the_two() {
        assert!(matches!(
            authorize_replay(&InputProvenance::undeclared(), false, None),
            Err(BridgeError::ReplayNeedsProvenance)
        ));
    }

    #[test]
    fn an_undeclared_input_replays_with_an_attestation() {
        let auth = authorize_replay(
            &InputProvenance::undeclared(),
            false,
            Some("WN586X3/24.10.8"),
        )
        .unwrap();
        assert_eq!(
            auth,
            ReplayAuthorization::Attested {
                captured_on: "WN586X3/24.10.8".to_string()
            }
        );
    }

    #[test]
    fn an_attestation_must_name_both_model_and_firmware() {
        for bad in ["WN586X3", "WN586X3/", "/24.10.8", "  /  ", ""] {
            assert!(
                matches!(
                    authorize_replay(&InputProvenance::undeclared(), false, Some(bad)),
                    Err(BridgeError::BadAttestation(_))
                ),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn the_filename_infix_alone_declares_synthetic() {
        let p = Path::new("/tmp/dump-mt7981.synthetic.json");
        assert!(file_name_declares_synthetic(p));
        assert!(!file_name_declares_synthetic(Path::new("/tmp/dump.json")));
    }

    #[test]
    fn sidecar_path_appends_rather_than_replacing_the_extension() {
        assert_eq!(
            sidecar_path(Path::new("/a/b/dump.synthetic.json")),
            PathBuf::from("/a/b/dump.synthetic.json.provenance.json")
        );
    }

    #[test]
    fn a_capture_header_can_add_synthetic_but_never_clear_it() {
        let added = merge_capture_header(InputProvenance::undeclared(), true);
        assert!(added.synthetic);
        assert_eq!(added.evidence, "capture header");

        let kept = merge_capture_header(synthetic_input(), false);
        assert!(kept.synthetic, "a header must not clear a sidecar claim");
        assert_eq!(kept.evidence, "sidecar .provenance.json");
    }
}
