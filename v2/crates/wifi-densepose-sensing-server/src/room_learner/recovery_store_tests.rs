//! Synthetic keys and temporary owned directories only.
use super::*;
const NOW: u64 = 1_800_000_000_000;
fn fixture(seed: Option<&[u8; 32]>) -> (tempfile::TempDir, RecoveryStore) {
    let t = tempfile::tempdir().unwrap();
    let s = RecoveryStore::new(
        &t.path().join("a".repeat(64)).join("SensingServer"),
        &"a".repeat(64),
        "install",
        seed,
    )
    .unwrap();
    (t, s)
}
fn descriptor() -> RecoveryDescriptor {
    RecoveryDescriptor {
        schema: RECOVERY_SCHEMA.into(),
        id: "original-run".into(),
        owner_scope: "a".repeat(64),
        installation_id: "install".into(),
        context_digest: "b".repeat(64),
        created_at_ms: NOW,
        saved_at_ms: NOW,
        expires_at_ms: NOW + RECOVERY_TTL_MS,
    }
}
#[test]
fn binary_roundtrip_fresh_nonce_and_original_deadline() {
    let (_t, s) = fixture(Some(&[7; 32]));
    let mut d = descriptor();
    let payload = b"opaque u128=340282366920938463463374607431768211455\0\xff";
    s.save(&d, payload, NOW).unwrap();
    let before = fs::read(s.directory.join(FILE)).unwrap();
    let r = s.load(NOW + 1).unwrap().unwrap();
    assert_eq!(r.descriptor, d);
    assert_eq!(r.plaintext, payload);
    assert!(!before.windows(payload.len()).any(|w| w == payload));
    d.saved_at_ms += 1;
    s.save(&d, payload, NOW + 1).unwrap();
    let after = fs::read(s.directory.join(FILE)).unwrap();
    assert_ne!(before, after);
    assert_eq!(
        s.load(NOW + 2).unwrap().unwrap().descriptor.expires_at_ms,
        NOW + RECOVERY_TTL_MS
    );
}
#[test]
fn unavailable_never_writes_plaintext() {
    let (_t, s) = fixture(None);
    assert!(!s.available());
    assert_eq!(
        s.save(&descriptor(), b"secret", NOW).unwrap_err(),
        "recovery_unavailable"
    );
    assert!(s.load(NOW).is_err());
    assert!(!s.directory.join(FILE).exists());
}
#[test]
fn identity_context_deadline_and_clock_cannot_be_replaced_or_renewed() {
    let (_t, s) = fixture(Some(&[7; 32]));
    let d = descriptor();
    s.save(&d, b"old", NOW).unwrap();
    let path = s.directory.join(FILE);
    let before = fs::read(&path).unwrap();
    for field in [
        "id", "context", "created", "expires", "saved", "owner", "install", "schema",
    ] {
        let mut bad = d.clone();
        match field {
            "id" => bad.id = "new".into(),
            "context" => bad.context_digest = "c".repeat(64),
            "created" => bad.created_at_ms += 1,
            "expires" => bad.expires_at_ms -= 1,
            "saved" => bad.saved_at_ms -= 1,
            "owner" => bad.owner_scope = "d".repeat(64),
            "install" => bad.installation_id = "other".into(),
            _ => bad.schema = "future".into(),
        };
        assert!(s.save(&bad, b"new", NOW + 5).is_err(), "{field}");
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(s.load(NOW - 1).unwrap_err_string().contains("clock"));
    assert!(s.discard("wrong-id").is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}
// Avoid requiring Debug on a value containing plaintext.
trait ErrorText {
    fn unwrap_err_string(self) -> String;
}
impl<T> ErrorText for Result<T, String> {
    fn unwrap_err_string(self) -> String {
        match self {
            Err(e) => e,
            Ok(_) => panic!("unexpected success"),
        }
    }
}
#[test]
fn expiry_retained_and_explicit_authenticated_discard() {
    let (_t, s) = fixture(Some(&[7; 32]));
    s.save(&descriptor(), b"bytes", NOW).unwrap();
    let path = s.directory.join(FILE);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        s.load(NOW + RECOVERY_TTL_MS).unwrap_err_string(),
        "recovery_expired"
    );
    let mut next = descriptor();
    next.saved_at_ms = NOW + 1;
    assert_eq!(
        s.save(&next, b"new", NOW + RECOVERY_TTL_MS).unwrap_err(),
        "recovery_expired"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    s.discard("original-run").unwrap();
    assert!(!path.exists());
    s.save(&descriptor(), b"new", NOW).unwrap();
}
#[test]
fn corruption_descriptor_nonce_tag_and_ciphertext_fail_closed() {
    for position in [0, 8, 12, 50, 350, 400] {
        let (_t, s) = fixture(Some(&[7; 32]));
        s.save(&descriptor(), &[42; 512], NOW).unwrap();
        let path = s.directory.join(FILE);
        let mut bytes = fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[position % n] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(s.load(NOW).is_err(), "position {position}");
        assert!(s.save(&descriptor(), b"new", NOW).is_err());
        assert!(s.discard("original-run").is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
#[test]
fn wrong_key_and_installation_cannot_read_or_overwrite() {
    let (_t, s) = fixture(Some(&[7; 32]));
    s.save(&descriptor(), b"opaque", NOW).unwrap();
    let path = s.directory.join(FILE);
    let before = fs::read(&path).unwrap();
    let data = s.directory.parent().unwrap();
    for (seed, installation) in [([8; 32], "install"), ([7; 32], "other-install")] {
        let other = RecoveryStore::new(data, &"a".repeat(64), installation, Some(&seed)).unwrap();
        assert!(other.load(NOW).is_err());
        let mut d = descriptor();
        d.installation_id = installation.into();
        assert!(other.save(&d, b"bad", NOW).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(RecoveryStore::new(data, &"c".repeat(64), "install", Some(&[7; 32])).is_err());
}
#[test]
fn bounded_large_payload_and_oversize_file_fail_closed() {
    let (_t, s) = fixture(Some(&[7; 32]));
    assert_eq!(
        s.save(&descriptor(), &vec![1; MAX_RECOVERY_BYTES + 1], NOW)
            .unwrap_err(),
        "recovery_budget_exceeded"
    );
    assert!(!s.directory.join(FILE).exists());
    s.save(&descriptor(), b"small", NOW).unwrap();
    let f = OpenOptions::new()
        .write(true)
        .open(s.directory.join(FILE))
        .unwrap();
    f.set_len((MAX_FILE + 1) as u64).unwrap();
    assert!(s.load(NOW).is_err());
    assert!(s.save(&descriptor(), b"new", NOW).is_err());
}
#[cfg(unix)]
#[test]
fn private_files_symlinks_and_owner_witness_revalidated() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for target in ["file", "directory", "witness"] {
        let (t, s) = fixture(Some(&[7; 32]));
        s.save(&descriptor(), b"small", NOW).unwrap();
        let path = match target {
            "file" => s.directory.join(FILE),
            "directory" => s.directory.clone(),
            _ => s.directory.parent().unwrap().join("account-owner-v1.json"),
        };
        fs::rename(&path, path.with_extension("preserved")).unwrap();
        let victim = t.path().join("victim");
        fs::write(&victim, b"untouched").unwrap();
        fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&victim, &path).unwrap();
        assert!(s.load(NOW).is_err());
        assert!(s.save(&descriptor(), b"bad", NOW).is_err());
        assert_eq!(fs::read(victim).unwrap(), b"untouched");
    }
    let (_t, s) = fixture(Some(&[7; 32]));
    s.save(&descriptor(), b"small", NOW).unwrap();
    let p = s.directory.join(FILE);
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(s.load(NOW).is_err());
    assert!(s.save(&descriptor(), b"bad", NOW).is_err());
}
#[test]
fn invalid_paths_and_descriptor_bounds_do_not_disclose_key_or_payload() {
    let t = tempfile::tempdir().unwrap();
    assert!(RecoveryStore::new(
        Path::new("relative"),
        &"a".repeat(64),
        "install",
        Some(&[7; 32])
    )
    .is_err());
    let (_t, s) = fixture(Some(&[7; 32]));
    let mut d = descriptor();
    d.id = "../escape".into(); // IDs are opaque authenticated metadata, never paths.
    s.save(&d, b"sensitive", NOW).unwrap();
    assert!(!t.path().join("escape").exists());
    d.id = "x".repeat(257);
    let e = s.save(&d, b"sensitive", NOW).unwrap_err();
    assert!(!e.contains("sensitive"));
    assert!(!e.contains("070707"));
}

#[test]
fn managed_key_policy_has_no_implicit_dev_or_ephemeral_fallback() {
    let value = "07".repeat(32);
    for stable in [None, Some(""), Some("true"), Some("0"), Some("1 ")] {
        assert!(managed_seed(stable, Some(&value)).is_none());
    }
    for invalid in [
        None,
        Some("short"),
        Some("adr262-ruview-rufield-dev-seed!!"),
    ] {
        assert!(managed_seed(Some("1"), invalid).is_none());
    }
    assert_eq!(managed_seed(Some("1"), Some(&value)), Some([7; 32]));
    assert!(managed_seed(Some("1"), Some(&"z".repeat(64))).is_none());
}
#[test]
fn copied_other_owner_ciphertext_does_not_authenticate() {
    let (_a, a) = fixture(Some(&[7; 32]));
    a.save(&descriptor(), b"private-owner-a", NOW).unwrap();
    let b_root = tempfile::tempdir().unwrap();
    let b = RecoveryStore::new(
        &b_root.path().join("b".repeat(64)).join("SensingServer"),
        &"b".repeat(64),
        "install",
        Some(&[7; 32]),
    )
    .unwrap();
    fs::copy(a.directory.join(FILE), b.directory.join(FILE)).unwrap();
    assert!(b.load(NOW).is_err());
    let mut d = descriptor();
    d.owner_scope = "b".repeat(64);
    assert!(b.save(&d, b"bad", NOW).is_err());
}
#[cfg(unix)]
#[test]
fn owner_permissions_drift_is_not_silently_repaired() {
    use std::os::unix::fs::PermissionsExt;
    for witness in [true, false] {
        let (_t, s) = fixture(Some(&[7; 32]));
        s.save(&descriptor(), b"original", NOW).unwrap();
        let path = if witness {
            s.directory.parent().unwrap().join("account-owner-v1.json")
        } else {
            s.directory.parent().unwrap().to_path_buf()
        };
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(if witness { 0o644 } else { 0o755 }),
        )
        .unwrap();
        assert!(s.load(NOW).is_err());
        assert!(s.save(&descriptor(), b"bad", NOW).is_err());
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            if witness { 0o644 } else { 0o755 }
        );
    }
}

#[test]
fn descriptor_nonce_and_tag_are_all_authenticated_and_truncation_is_retained() {
    for part in ["descriptor", "nonce", "ciphertext", "tag", "truncated"] {
        let (_t, s) = fixture(Some(&[7; 32]));
        s.save(&descriptor(), b"bounded opaque payload", NOW)
            .unwrap();
        let path = s.directory.join(FILE);
        let mut bytes = fs::read(&path).unwrap();
        let descriptor_len = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
        match part {
            "descriptor" => {
                let at = bytes
                    .windows(12)
                    .position(|w| w == b"original-run")
                    .unwrap();
                bytes[at] = b'O';
            }
            "nonce" => bytes[12 + descriptor_len] ^= 1,
            "ciphertext" => bytes[24 + descriptor_len] ^= 1,
            "tag" => {
                let end = bytes.len() - 1;
                bytes[end] ^= 1;
            }
            _ => {
                bytes.truncate(bytes.len() - 1);
            }
        }
        fs::write(&path, &bytes).unwrap();
        assert!(s.load(NOW).is_err(), "{part}");
        assert!(s.save(&descriptor(), b"replacement", NOW).is_err());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}
#[test]
fn exact_plaintext_budget_roundtrips() {
    let (_t, s) = fixture(Some(&[7; 32]));
    let bytes = vec![0x65; MAX_RECOVERY_BYTES];
    s.save(&descriptor(), &bytes, NOW).unwrap();
    assert_eq!(s.load(NOW).unwrap().unwrap().plaintext, bytes);
}

#[test]
fn expired_authenticated_inspection_preserves_deadline_then_explicit_discard() {
    let (_t, s) = fixture(Some(&[7; 32]));
    let d = descriptor();
    s.save(&d, b"private reduced samples", NOW).unwrap();
    let path = s.directory.join(FILE);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        s.load(d.expires_at_ms).unwrap_err_string(),
        "recovery_expired"
    );
    assert_eq!(s.inspect().unwrap(), Some(d.clone()));
    assert!(s.load(NOW - 1).is_err());
    assert_eq!(s.inspect().unwrap(), Some(d));
    assert_eq!(fs::read(&path).unwrap(), before);
    s.discard("original-run").unwrap();
    assert_eq!(s.inspect().unwrap(), None);
}
#[test]
fn wrong_key_or_tampered_inspection_fails_without_altering_retained_evidence() {
    let (_t, s) = fixture(Some(&[7; 32]));
    s.save(&descriptor(), b"private reduced samples", NOW)
        .unwrap();
    let path = s.directory.join(FILE);
    let before = fs::read(&path).unwrap();
    let other = RecoveryStore::new(
        s.directory.parent().unwrap(),
        &"a".repeat(64),
        "install",
        Some(&[8; 32]),
    )
    .unwrap();
    assert!(other.inspect().is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut bad = before;
    let last = bad.len() - 1;
    bad[last] ^= 1;
    fs::write(&path, &bad).unwrap();
    assert!(s.inspect().is_err());
    assert_eq!(fs::read(&path).unwrap(), bad);
}

#[test]
fn operator_profile_scope_preserves_old_bytes_and_uses_a_separate_key_without_recording_consent() {
    let (_t,old)=fixture(Some(&[7;32]));
    old.save(&descriptor(),b"synthetic previous-plan evidence",NOW).unwrap();
    let bytes=fs::read(old.directory.join(FILE)).unwrap();
    let new=old.operator_eight_scope().unwrap();
    assert!(new.available());
    assert!(fs::read_dir(&new.directory).unwrap().next().is_none());
    assert!(new.inspect().unwrap().is_none());
    new.save(&descriptor(),b"synthetic eight-step evidence",NOW).unwrap();
    assert_eq!(new.load(NOW+1).unwrap().unwrap().plaintext,b"synthetic eight-step evidence");
    assert_eq!(fs::read(old.directory.join(FILE)).unwrap(),bytes);
    assert_eq!(old.load(NOW+1).unwrap().unwrap().plaintext,b"synthetic previous-plan evidence");
    // The same owner's old ciphertext cannot authenticate in the new key domain.
    let valid_new=fs::read(new.directory.join(FILE)).unwrap();
    fs::write(new.directory.join(FILE),&bytes).unwrap();
    assert!(new.load(NOW+1).is_err());
    fs::write(new.directory.join(FILE),valid_new).unwrap();
    assert!(new.operator_eight_scope().is_err());
}
