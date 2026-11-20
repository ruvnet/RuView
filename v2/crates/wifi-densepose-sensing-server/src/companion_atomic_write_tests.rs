//! Actual canonical writer boundary controls; no model/selection permission is
//! manufactured by these generic byte fixtures.
use super::*;
use std::cell::Cell;
#[test]
fn preinstall_refusal_after_synced_staging_preserves_pointer_and_cleans_own_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pointer.json");
    atomic_private_write(&path, b"old-choice").unwrap();
    let observed = Cell::new(false);
    let result = atomic_private_write_checked(
        &path,
        b"new-choice",
        &mut || {
            assert_eq!(std::fs::read(&path)?, b"old-choice");
            let staging = std::fs::read_dir(dir.path())?
                .map(|e| e.unwrap().path())
                .filter(|p| p != &path)
                .collect::<Vec<_>>();
            assert_eq!(staging.len(), 1);
            assert_eq!(std::fs::read(&staging[0])?, b"new-choice");
            observed.set(true);
            Err(io::Error::other(
                "synthetic-current-lease-expired-before-rename",
            ))
        },
        &mut || panic!("refused staging reached authoritative rename"),
    );
    assert!(result.is_err());
    assert!(observed.get());
    assert_eq!(std::fs::read(&path).unwrap(), b"old-choice");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn admitted_rename_then_ambiguous_sync_retains_exact_choice_for_cold_readback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pointer.json");
    atomic_private_write(&path, b"old-choice").unwrap();
    let admitted = Cell::new(false);
    let result = atomic_private_write_checked(
        &path,
        b"original-prepared-choice",
        &mut || {
            assert_eq!(std::fs::read(&path)?, b"old-choice");
            admitted.set(true);
            Ok(())
        },
        &mut || {
            assert!(admitted.get());
            assert_eq!(std::fs::read(&path)?, b"original-prepared-choice");
            Err(io::Error::other(
                "synthetic-directory-sync-failure-after-valid-rename",
            ))
        },
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original-prepared-choice");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn original_noop_wrapper_keeps_private_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pointer.json");
    atomic_private_write(&path, b"unchanged-canonical-writer").unwrap();
    assert_eq!(private_read(&path).unwrap(), b"unchanged-canonical-writer");
}
