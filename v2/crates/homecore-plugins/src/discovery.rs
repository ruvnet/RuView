//! Secure filesystem discovery for packaged WASM plugins.
//!
//! A plugin directory has exactly this shape:
//!
//! ```text
//! <configured-root>/<package>/manifest.json
//!                              <module named by wasm_module>
//! ```
//!
//! Only immediate child directories of explicitly configured roots are
//! inspected. Symlinks are rejected and the module must be a plain filename
//! in the same package directory. This deliberately avoids recursive search,
//! implicit current-directory loading, and path traversal.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{PluginError, PluginManifest};

/// Conservative input limits applied before allocating file contents.
#[derive(Debug, Clone, Copy)]
pub struct DiscoveryLimits {
    pub max_plugins: usize,
    pub max_manifest_bytes: u64,
    pub max_module_bytes: u64,
}

impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            max_plugins: 128,
            max_manifest_bytes: 256 * 1024,
            max_module_bytes: 16 * 1024 * 1024,
        }
    }
}

/// A validated package whose files are safe to read.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub manifest: PluginManifest,
    pub package_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub module_path: PathBuf,
}

impl DiscoveredPlugin {
    /// Read the module after re-checking its type and size. Re-checking closes
    /// the common accidental replacement window between discovery and load;
    /// signature verification remains the authoritative tamper gate.
    pub fn read_module(&self, limits: DiscoveryLimits) -> Result<Vec<u8>, PluginError> {
        bounded_regular_file(&self.module_path, limits.max_module_bytes, "WASM module")
    }
}

/// Discover packages in deterministic root/path order.
pub fn discover_plugins(
    roots: &[PathBuf],
    limits: DiscoveryLimits,
) -> Result<Vec<DiscoveredPlugin>, PluginError> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    if limits.max_plugins == 0 || limits.max_manifest_bytes == 0 || limits.max_module_bytes == 0 {
        return Err(PluginError::ResourceLimit(
            "discovery limits must all be greater than zero".into(),
        ));
    }

    let mut packages = BTreeSet::new();
    for configured_root in roots {
        let metadata = fs::symlink_metadata(configured_root).map_err(|e| {
            PluginError::Discovery(format!(
                "cannot inspect configured plugin root {}: {e}",
                configured_root.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(PluginError::Discovery(format!(
                "configured plugin root must be a real directory, not a symlink: {}",
                configured_root.display()
            )));
        }
        let root = fs::canonicalize(configured_root)?;
        let entries = fs::read_dir(&root)?;
        for entry in entries {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_symlink() {
                return Err(PluginError::Discovery(format!(
                    "symlink found in plugin root: {}",
                    entry.path().display()
                )));
            }
            if ty.is_dir() && entry.path().join("manifest.json").exists() {
                packages.insert(entry.path());
            }
        }
    }

    if packages.len() > limits.max_plugins {
        return Err(PluginError::ResourceLimit(format!(
            "discovered {} plugins, maximum is {}",
            packages.len(),
            limits.max_plugins
        )));
    }

    let mut result = Vec::with_capacity(packages.len());
    let mut domains = BTreeSet::new();
    for package_dir in packages {
        let package_dir = fs::canonicalize(package_dir)?;
        let manifest_path = package_dir.join("manifest.json");
        let manifest_bytes =
            bounded_regular_file(&manifest_path, limits.max_manifest_bytes, "manifest")?;
        let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|e| {
            PluginError::InvalidManifest(format!("{} is not UTF-8: {e}", manifest_path.display()))
        })?;
        let manifest = PluginManifest::parse_json(manifest_text)?;
        if !domains.insert(manifest.domain.clone()) {
            return Err(PluginError::Discovery(format!(
                "duplicate plugin domain `{}`",
                manifest.domain
            )));
        }
        let module_name = manifest.wasm_module.as_deref().ok_or_else(|| {
            PluginError::InvalidManifest(format!(
                "WASM package `{}` has no wasm_module",
                manifest.domain
            ))
        })?;
        let relative = Path::new(module_name);
        if relative.components().count() != 1
            || !matches!(relative.components().next(), Some(Component::Normal(_)))
            || relative.extension().and_then(|v| v.to_str()) != Some("wasm")
        {
            return Err(PluginError::InvalidManifest(format!(
                "plugin `{}` wasm_module must be a .wasm filename in its package directory",
                manifest.domain
            )));
        }
        let module_path = package_dir.join(relative);
        let metadata = fs::symlink_metadata(&module_path).map_err(|e| {
            PluginError::Discovery(format!("cannot inspect {}: {e}", module_path.display()))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PluginError::Discovery(format!(
                "plugin module must be a regular non-symlink file: {}",
                module_path.display()
            )));
        }
        if metadata.len() > limits.max_module_bytes {
            return Err(PluginError::ResourceLimit(format!(
                "{} is {} bytes; maximum is {}",
                module_path.display(),
                metadata.len(),
                limits.max_module_bytes
            )));
        }
        result.push(DiscoveredPlugin {
            manifest,
            package_dir,
            manifest_path,
            module_path,
        });
    }
    Ok(result)
}

fn bounded_regular_file(path: &Path, maximum: u64, kind: &str) -> Result<Vec<u8>, PluginError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PluginError::Discovery(format!(
            "{kind} must be a regular non-symlink file: {}",
            path.display()
        )));
    }
    if metadata.len() > maximum {
        return Err(PluginError::ResourceLimit(format!(
            "{} is {} bytes; maximum is {}",
            path.display(),
            metadata.len(),
            maximum
        )));
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > maximum {
        return Err(PluginError::ResourceLimit(format!(
            "{} grew beyond the {maximum}-byte limit while being read",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "homecore-plugin-discovery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn package(root: &Path, directory: &str, domain: &str, module: &str) {
        let dir = root.join(directory);
        fs::create_dir(&dir).unwrap();
        fs::write(
            dir.join("manifest.json"),
            format!(
                r#"{{"domain":"{domain}","name":"Test","version":"1","wasm_module":"{module}"}}"#
            ),
        )
        .unwrap();
        if !module.contains('/') && !module.contains('\\') && module.ends_with(".wasm") {
            fs::write(dir.join(module), b"\0asm\x01\0\0\0").unwrap();
        }
    }

    #[test]
    fn discovery_is_sorted_and_only_reads_explicit_roots() {
        let root = root();
        package(&root, "z-last", "zeta", "z.wasm");
        package(&root, "a-first", "alpha", "a.wasm");
        let found =
            discover_plugins(std::slice::from_ref(&root), DiscoveryLimits::default()).unwrap();
        assert_eq!(
            found
                .iter()
                .map(|p| p.manifest.domain.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_module_path_is_rejected() {
        let root = root();
        package(&root, "bad", "bad", "../bad.wasm");
        let error =
            discover_plugins(std::slice::from_ref(&root), DiscoveryLimits::default()).unwrap_err();
        assert!(matches!(error, PluginError::InvalidManifest(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn size_limits_are_enforced_before_read() {
        let root = root();
        package(&root, "large", "large", "large.wasm");
        let error = discover_plugins(
            std::slice::from_ref(&root),
            DiscoveryLimits {
                max_module_bytes: 4,
                ..DiscoveryLimits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, PluginError::ResourceLimit(_)));
        fs::remove_dir_all(root).unwrap();
    }
}
