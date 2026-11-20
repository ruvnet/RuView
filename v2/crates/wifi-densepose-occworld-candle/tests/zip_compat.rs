//! SYNTHETIC IO fixtures for Candle's actual ZIP-backed tensor API.

use candle_core::{Device, Tensor};

#[test]
fn candle_npz_roundtrip_preserves_tensor_names_shapes_and_values() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("fixture.npz");
    let tensor = Tensor::new(&[[1.0_f32, 2.0], [3.0, 4.0]], &Device::Cpu).unwrap();
    Tensor::write_npz(&[("fixture", &tensor)], &file).unwrap();
    let tensors = Tensor::read_npz(&file).unwrap();
    assert_eq!(tensors.len(), 1);
    assert_eq!(tensors[0].0, "fixture");
    assert_eq!(tensors[0].1.dims(), &[2, 2]);
    assert_eq!(
        tensors[0].1.to_vec2::<f32>().unwrap(),
        vec![vec![1.0, 2.0], vec![3.0, 4.0]]
    );
    let named = Tensor::read_npz_by_name(&file, &["fixture"]).unwrap();
    assert_eq!(
        named[0].to_vec2::<f32>().unwrap(),
        tensors[0].1.to_vec2::<f32>().unwrap()
    );
}

#[test]
fn candle_rejects_missing_named_entries_and_truncated_archives() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("fixture.npz");
    let tensor = Tensor::new(&[1.0_f32, 2.0], &Device::Cpu).unwrap();
    Tensor::write_npz(&[("fixture", &tensor)], &file).unwrap();
    assert!(Tensor::read_npz_by_name(&file, &["missing"]).is_err());
    let mut bytes = std::fs::read(&file).unwrap();
    bytes.truncate(bytes.len() / 2);
    std::fs::write(&file, bytes).unwrap();
    assert!(Tensor::read_npz(&file).is_err());
}
