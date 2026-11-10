//! CPU-only regression coverage for the patched Candle/ZIP dependency boundary.
use candle_core::{npy::NpzTensors, Device, Tensor};

#[test]
fn npz_eager_and_lazy_readers_preserve_tensor_names_and_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("weights.npz");
    let tensor = Tensor::new(&[1.0f32, -2.5, 3.25], &Device::Cpu).unwrap();
    Tensor::write_npz(&[("room_µ", &tensor)], &path).unwrap();

    let eager = Tensor::read_npz(&path).unwrap();
    assert_eq!(eager.len(), 1);
    assert_eq!(eager[0].0, "room_µ");
    assert_eq!(eager[0].1.to_vec1::<f32>().unwrap(), [1.0, -2.5, 3.25]);

    let lazy = NpzTensors::new(&path).unwrap();
    assert_eq!(
        lazy.get("room_µ")
            .unwrap()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap(),
        [1.0, -2.5, 3.25]
    );
    assert!(lazy.get("absent").unwrap().is_none());
    // The PTH metadata reader can enumerate this valid archive, which has no
    // data.pkl member; it should find no PyTorch tensors, not a decoding error.
    assert!(
        candle_core::pickle::read_pth_tensor_info(&path, false, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn archive_readers_preserve_legacy_cp437_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy-name.npz");
    let tensor = Tensor::new(&[1.0f32], &Device::Cpu).unwrap();
    Tensor::write_npz(&[("weights", &tensor)], &path).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    // Replace an ASCII name with a legacy CP437 name in a local one-entry ZIP.
    // The array payload, CRC, sizes and offsets stay valid.
    let eocd = bytes.len() - 22;
    assert_eq!(&bytes[eocd..eocd + 4], b"PK\x05\x06");
    let central = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(&bytes[central..central + 4], b"PK\x01\x02");
    for flags in [6, central + 8] {
        bytes[flags + 1] &= !0x08;
    }
    bytes[30] = 0x82; // CP437 e-acute, which is not a standalone UTF-8 byte.
    bytes[central + 46] = 0x82;
    std::fs::write(&path, bytes).unwrap();

    let tensors = Tensor::read_npz(&path).unwrap();
    assert_eq!(tensors[0].0, "éeights");
    assert_eq!(tensors[0].1.to_vec1::<f32>().unwrap(), [1.0]);
    let lazy = NpzTensors::new(&path).unwrap();
    assert_eq!(
        lazy.get("éeights")
            .unwrap()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap(),
        [1.0]
    );
    assert!(
        candle_core::pickle::read_pth_tensor_info(&path, false, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn archive_readers_return_errors_for_truncated_headers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.npz");
    std::fs::write(&path, b"PK\x03\x04").unwrap();
    assert!(Tensor::read_npz(&path).is_err());
    assert!(NpzTensors::new(&path).is_err());
    assert!(candle_core::pickle::read_pth_tensor_info(&path, false, None).is_err());
}
