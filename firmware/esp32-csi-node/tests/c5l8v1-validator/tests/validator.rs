//! Producer -> real validator contract test for C5L8/v1.
//! The C harness is compiled from the firmware's own c5_lltf8.c; nothing is re-implemented here.
use ruview_c5l8v1_check::{hex_decode, validate_c5l8};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use wifi_densepose_sensing_server::room_learner::native::{C5_LLTF53_TONES, C5_LLTF8_V1_GRID};

const SUFFIX_NO_FWI: [u8; 16] = [
    b'C', b'5', b'L', b'8', 1, 8, 1, 1, 1, 1, 0, 20, 1, 0, 106, 0,
];
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"),
    "/validator-src/v2/crates/wifi-densepose-sensing-server/tests/fixtures/c5-node23-wire-metadata-20261110.json");

type Cases = BTreeMap<String, (String, Vec<u8>)>;

/// Compiled and run exactly once per test binary; parallel tests share the result.
fn emitter_output() -> Cases {
    static CASES: std::sync::OnceLock<Cases> = std::sync::OnceLock::new();
    CASES.get_or_init(build_and_run_emitter).clone()
}

fn build_and_run_emitter() -> Cases {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let main = root.join("../../main");
    let exe = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("c5l8-emitter");
    let st = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O1", "-I"])
        .arg(&main)
        .arg(main.join("c5_lltf8.c"))
        .arg(root.join("harness/emitter.c"))
        .arg("-o")
        .arg(&exe)
        .status()
        .expect("cc");
    assert!(st.success(), "emitter failed to compile");
    let out = Command::new(&exe).output().expect("run emitter");
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let mut it = l.split_whitespace();
            let (name, status, hex) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let wire = if hex == "-" {
                vec![]
            } else {
                hex_decode(hex).unwrap()
            };
            (name.to_string(), (status.to_string(), wire))
        })
        .collect()
}

#[test]
fn producer_frames_pass_the_real_validator_and_canonicalizer() {
    let cases = emitter_output();
    let ok: Vec<_> = cases
        .iter()
        .filter(|(n, _)| n.starts_with("ok_") || n.starts_with("selftest_"))
        .collect();
    assert_eq!(ok.len(), 14);
    for (name, (status, wire)) in ok {
        assert_eq!(status, "ok", "{name}");
        assert_eq!(wire.len(), 142, "{name}");
        assert_eq!(&wire[..4], &0xC5110001u32.to_le_bytes(), "{name}");
        assert_eq!(
            (wire[5], u16::from_le_bytes([wire[6], wire[7]]), wire[18]),
            (1, 53, 0),
            "{name}"
        );
        let packet = validate_c5l8(wire).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(packet.grid_id, C5_LLTF8_V1_GRID);
        assert_eq!(
            packet.native_iq_wire.as_deref(),
            Some(&wire[20..126]),
            "{name}: IQ bytes not lossless"
        );
        assert_eq!(packet.native_tone_indices.unwrap(), &C5_LLTF53_TONES);
        assert_eq!(packet.native_frame.native_shape, vec![1, 53, 1]);
        assert_eq!(packet.bandwidth_hz, 20_000_000);
        assert_eq!(packet.amplitudes.len(), 56);
        assert!(!packet.phase_consumed);
        assert!(
            !packet.native_frame.valid_mask[0],
            "{name}: DC slot must be invalid"
        );
        let sanitized = wire[19] & 0x20 != 0;
        assert_eq!(wire[139] == 1, sanitized, "{name}");
        if sanitized {
            assert_eq!(&wire[20..24], &[0, 0, 0, 0]);
            assert!(!packet.native_frame.valid_mask[1]);
        }
        let mut expect = SUFFIX_NO_FWI;
        expect[13] = u8::from(sanitized);
        assert_eq!(&wire[126..], &expect, "{name}: suffix");
    }
}

#[test]
fn default_frame_matches_the_node23_wire_metadata_fixture() {
    let receipt: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(FIXTURE).unwrap()).unwrap();
    let m = &receipt["variants"][0]["metadata"];
    let s = &m["c5_suffix_metadata"];
    let cases = emitter_output();
    for name in ["ok_5240_sync", "selftest_0"] {
        let wire = &cases[name].1;
        assert_eq!(wire.len() as u64, m["wire_bytes"].as_u64().unwrap());
        assert_eq!(
            u64::from(wire[19]),
            m["flags_byte"].as_u64().unwrap(),
            "{name}: header flag 0x10"
        );
        assert_eq!(
            u64::from(u32::from_le_bytes(wire[8..12].try_into().unwrap())),
            m["frequency_mhz"].as_u64().unwrap()
        );
        assert_eq!(u64::from(wire[18]), m["ppdu_byte"].as_u64().unwrap());
        assert_eq!(u64::from(wire[5]), m["antennas"].as_u64().unwrap());
        assert_eq!(
            u64::from(u16::from_le_bytes([wire[6], wire[7]])),
            m["declared_native_slots"].as_u64().unwrap()
        );
        let sf = &wire[126..];
        assert_eq!(
            std::str::from_utf8(&sf[..4]).unwrap(),
            s["marker"].as_str().unwrap()
        );
        for (i, key) in [
            "version",
            "component_bits",
            "component_order",
            "tone_order",
            "sdk_readback",
            "bb_format",
            "secondary_channel",
            "sta_bandwidth_mhz",
            "estimator_valid",
            "first_word_invalid",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(
                u64::from(sf[4 + i]),
                s[*key].as_u64().unwrap(),
                "{name}: suffix {key}"
            );
        }
        assert_eq!(
            u64::from(u16::from_le_bytes([sf[14], sf[15]])),
            s["declared_iq_bytes"].as_u64().unwrap()
        );
        // The fixture records the OLD strict decoder rejecting 0x10; the pinned decoder must accept it.
        validate_c5l8(wire).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn producer_withholds_every_unclaimable_callback() {
    let cases = emitter_output();
    let expect = [
        ("no_readback", "csi_config_readback_mismatch"),
        ("bw40", "sta_bandwidth_not_20mhz"),
        ("ht", "not_non_ht_ofdm"),
        ("he_su", "not_non_ht_ofdm"),
        ("dsss_11b", "not_non_ht_ofdm"),
        ("secondary", "secondary_channel_present"),
        ("estimate_invalid", "estimate_invalid"),
        ("len128", "callback_len_not_106"),
        ("len104", "callback_len_not_106"),
        ("ch6_2g4", "unsupported_5ghz_primary"),
        ("ch38_offgrid", "unsupported_5ghz_primary"),
        ("ch181", "unsupported_5ghz_primary"),
    ];
    for (name, reason) in expect {
        let (status, wire) = &cases[name];
        assert_eq!(status, reason, "{name}");
        assert!(wire.is_empty(), "{name}: no samples on refusal");
    }
    assert_eq!(cases.len(), 14 + expect.len());
}

#[test]
fn validator_rejects_mutations_of_a_producer_frame() {
    // Guards against a vacuous pass: the same validator must refuse near-misses.
    let wire = emitter_output()["ok_5240_sync"].1.clone();
    assert!(
        validate_c5l8(&wire[..126]).is_err(),
        "bare 106-byte 53-slot frame"
    );
    let mut m = wire.clone();
    m.push(0);
    assert!(validate_c5l8(&m).is_err(), "trailing byte");
    for (idx, val) in [
        (130usize, 2u8),
        (131, 12),
        (137, 40),
        (19, 0x11),
        (19, 0x20),
        (18, 1),
    ] {
        let mut m = wire.clone();
        m[idx] = val;
        assert!(validate_c5l8(&m).is_err(), "byte {idx}={val}");
    }
    let mut m = wire.clone();
    m[8..12].copy_from_slice(&2437u32.to_le_bytes());
    assert!(validate_c5l8(&m).is_err(), "2.4 GHz");
}
