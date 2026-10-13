//! Confidence validation at the deserialization boundary.
#![cfg(feature = "serde")]

use wifi_densepose_core::{Confidence, Keypoint};

#[test]
fn confidence_deserialization_rejects_out_of_range_values() {
    for value in [-1.0_f32, -f32::EPSILON, 1.0 + f32::EPSILON, 2.0] {
        let json = serde_json::to_string(&value).unwrap();
        assert!(
            serde_json::from_str::<Confidence>(&json).is_err(),
            "accepted {json}"
        );
    }
}

#[test]
fn confidence_deserialization_rejects_nonfinite_values() {
    use serde::de::value::{Error, F32Deserializer};
    use serde::Deserialize;

    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(Confidence::deserialize(F32Deserializer::<Error>::new(value)).is_err());
    }
}

#[test]
fn confidence_roundtrips_scalar_wire_format_and_default() {
    for value in [0.0_f32, 0.25, 0.5, 1.0] {
        let confidence = Confidence::new(value).unwrap();
        let json = serde_json::to_string(&confidence).unwrap();
        assert_eq!(json, serde_json::to_string(&value).unwrap());
        assert_eq!(
            serde_json::from_str::<Confidence>(&json).unwrap(),
            confidence
        );
    }
    assert_eq!(Confidence::default(), Confidence::MIN);
}

#[test]
fn keypoint_deserialization_validates_nested_confidence() {
    let json = r#"{"keypoint_type":"Nose","x":0.5,"y":0.3,"z":null,"confidence":2.0}"#;
    assert!(serde_json::from_str::<Keypoint>(json).is_err());
}
