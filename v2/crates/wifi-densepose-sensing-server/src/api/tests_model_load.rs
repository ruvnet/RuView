use std::path::Path;
use wifi_densepose_sensing_server::cli::handlers::{
    diagnose_model_load_error, export_emits_placeholder_demo, load_or_convert_model,
};

#[test]
fn jsonl_model_loads_through_model_flag_path() {
    let data = b"{\"model_id\":\"published\"}\n{\"weights\":[1.0,2.0]}\n";
    let mut loader = load_or_convert_model(Path::new("model.rvf.jsonl"), data)
        .expect("--model must auto-convert JSONL");
    loader.load_layer_a().expect("Layer A");
    let layer_c = loader.load_layer_c().expect("Layer C");
    assert_eq!(layer_c.all_weights, vec![1.0, 2.0]);
}

#[test]
fn safetensors_is_named_and_points_at_894() {
    let data = [0x10, 0, 0, 0, 0, 0, 0, 0, b'{', b'"'];
    let msg = diagnose_model_load_error(
        Path::new("models/wifi-densepose-pretrained/model.safetensors"),
        &data,
        "invalid magic at offset 0",
    );
    assert!(msg.contains("safetensors"), "{msg}");
    assert!(msg.contains("#894"), "{msg}");
    assert!(msg.contains("signal heuristics"), "{msg}");
}

#[test]
fn quantized_bin_is_identified() {
    let data = [0x35, 0x57, 0x45, 0x77];
    let msg = diagnose_model_load_error(Path::new("model-q4.bin"), &data, "bad magic");
    assert!(msg.contains("quantized weight blob"), "{msg}");
    assert!(msg.contains("RVFS") || msg.contains("0x52564653"), "{msg}");
}

#[test]
fn jsonl_manifest_is_identified() {
    let data = *b"{\"seg\":0}";
    let msg = diagnose_model_load_error(Path::new("model.rvf.jsonl"), &data, "x");
    assert!(msg.contains("JSONL manifest"), "{msg}");
}

#[test]
fn unknown_format_still_gives_guidance() {
    let data = [0u8, 1, 2, 3];
    let msg = diagnose_model_load_error(Path::new("weird.dat"), &data, "x");
    assert!(msg.contains("RVF binary container"), "{msg}");
    assert!(msg.contains("wifi-densepose-train"), "{msg}");
}

#[test]
fn standalone_export_emits_placeholder() {
    assert!(export_emits_placeholder_demo(true, false, false));
}

#[test]
fn export_with_train_does_not_short_circuit() {
    assert!(!export_emits_placeholder_demo(true, true, false));
    assert!(!export_emits_placeholder_demo(true, false, true));
    assert!(!export_emits_placeholder_demo(true, true, true));
}

#[test]
fn no_export_flag_never_emits() {
    assert!(!export_emits_placeholder_demo(false, false, false));
    assert!(!export_emits_placeholder_demo(false, true, false));
}

