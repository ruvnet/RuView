//! Private helper working directory and durable account ownership witness.
//! Profile selection comes from the authenticated native app, never an HTTP
//! request. Unknown/nonempty legacy data cannot become a new account's data.
use std::{
    io,
    path::{Path, PathBuf},
};

const OWNER_FILE: &str = "account-owner-v1.json";
const SCHEMA: &str = "ruview.account-owner.v1";

/// A desktop-owned helper cannot inherit a device-wide model mount. Manual
/// unowned server deployments retain their existing MODELS_DIR contract.
pub fn models_directory(
    owned: bool,
    data_dir: &Path,
    override_dir: Option<&std::ffi::OsStr>,
) -> PathBuf {
    if owned {
        data_dir.join("models")
    } else {
        override_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.join("models"))
    }
}

pub fn prepare_directory(scope: &str, directory: &Path) -> io::Result<PathBuf> {
    if scope.len() != 64
        || !scope
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !directory.is_absolute()
        || directory.file_name().and_then(|v| v.to_str()) != Some("SensingServer")
        || directory
            .parent()
            .and_then(Path::file_name)
            .and_then(|v| v.to_str())
            != Some(scope)
    {
        return Err(io::Error::other("invalid owned helper directory"));
    }
    if let Some(parent) = directory.parent().and_then(|path| path.canonicalize().ok()) {
        if parent.file_name().and_then(|v| v.to_str()) != Some(scope) {
            return Err(io::Error::other("account directory alias rejected"));
        }
    }
    crate::companion::private_dir(directory)?;
    let canonical = directory.canonicalize()?;
    if canonical
        .parent()
        .and_then(Path::file_name)
        .and_then(|v| v.to_str())
        != Some(scope)
    {
        return Err(io::Error::other(
            "helper directory belongs to another account namespace",
        ));
    }
    let witness = canonical.join(OWNER_FILE);
    match std::fs::symlink_metadata(&witness) {
        Ok(_) => {
            let value: serde_json::Value =
                serde_json::from_slice(&crate::companion::private_read(&witness)?)
                    .map_err(io::Error::other)?;
            if value.get("schema").and_then(|v| v.as_str()) != Some(SCHEMA)
                || value.get("scope_key").and_then(|v| v.as_str()) != Some(scope)
            {
                return Err(io::Error::other(
                    "helper account ownership witness mismatch",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if std::fs::read_dir(&canonical)?.next().is_some() {
                return Err(io::Error::other(
                    "existing helper data needs explicit owner migration",
                ));
            }
            let value =
                serde_json::to_vec(&serde_json::json!({"schema": SCHEMA, "scope_key": scope}))
                    .map_err(io::Error::other)?;
            crate::companion::atomic_private_write(&witness, &value)?;
            if crate::companion::private_read(&witness)? != value {
                return Err(io::Error::other("helper ownership readback failed"));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_models_ignore_a_device_wide_directory_override() {
        let a = Path::new("/fixture/account-A/models");
        let b = Path::new("/fixture/account-B/SensingServer");
        assert_eq!(
            models_directory(true, b, Some(a.as_os_str())),
            b.join("models")
        );
        assert_eq!(models_directory(false, b, Some(a.as_os_str())), a);
    }
    #[test]
    fn independent_profiles_keep_their_own_witness_and_legacy_is_not_adopted() {
        let root = tempfile::tempdir().unwrap();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let a_dir = root.path().join(&a).join("SensingServer");
        let b_dir = root.path().join(&b).join("SensingServer");
        let a_path = prepare_directory(&a, &a_dir).unwrap();
        let a_before = std::fs::read(a_path.join(OWNER_FILE)).unwrap();
        assert_eq!(prepare_directory(&a, &a_dir).unwrap(), a_path);
        prepare_directory(&b, &b_dir).unwrap();
        assert_eq!(std::fs::read(a_path.join(OWNER_FILE)).unwrap(), a_before);
        assert!(prepare_directory(&b, &a_dir).is_err());
        let legacy = root.path().join("c".repeat(64)).join("SensingServer");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("old-model.json"), "legacy bytes").unwrap();
        assert!(prepare_directory(&"c".repeat(64), &legacy).is_err());
        assert_eq!(
            std::fs::read_to_string(legacy.join("old-model.json")).unwrap(),
            "legacy bytes"
        );
        assert!(!legacy.join(OWNER_FILE).exists());
    }
    #[test]
    fn mismatched_or_future_witness_is_preserved_and_rejected() {
        let root = tempfile::tempdir().unwrap();
        let key = "a".repeat(64);
        let dir = prepare_directory(&key, &root.path().join(&key).join("SensingServer")).unwrap();
        let future = br#"{"schema":"unknown.future","scope_key":"unreadable"}"#;
        crate::companion::atomic_private_write(&dir.join(OWNER_FILE), future).unwrap();
        assert!(prepare_directory(&key, &dir).is_err());
        assert_eq!(std::fs::read(dir.join(OWNER_FILE)).unwrap(), future);
        assert!(prepare_directory("../legacy", &dir).is_err());
        assert!(prepare_directory(&key, Path::new("relative/SensingServer")).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_to_another_account_profile_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let a_dir = prepare_directory(&a, &root.path().join(&a).join("SensingServer")).unwrap();
        std::fs::create_dir(root.path().join(&b)).unwrap();
        std::os::unix::fs::symlink(&a_dir, root.path().join(&b).join("SensingServer")).unwrap();
        assert!(prepare_directory(&b, &root.path().join(&b).join("SensingServer")).is_err());
    }
}
