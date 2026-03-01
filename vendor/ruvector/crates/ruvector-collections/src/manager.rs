//! Collection manager for multi-collection operations

use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::collection::{Collection, CollectionConfig, CollectionStats};
use crate::error::{CollectionError, Result};

/// Metadata for persisting collections
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CollectionMetadata {
    name: String,
    config: CollectionConfig,
    created_at: i64,
    updated_at: i64,
}

/// Manages multiple vector collections with alias support
#[derive(Debug)]
pub struct CollectionManager {
    /// Active collections
    collections: DashMap<String, Arc<RwLock<Collection>>>,

    /// Alias mappings (alias -> collection_name)
    aliases: DashMap<String, String>,

    /// Base path for storing collections
    base_path: PathBuf,
}

impl CollectionManager {
    /// Create a new collection manager
    ///
    /// # Arguments
    ///
    /// * `base_path` - Directory where collections will be stored
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ruvector_collections::CollectionManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = CollectionManager::new(PathBuf::from("./collections")).unwrap();
    /// ```
    pub fn new(base_path: PathBuf) -> Result<Self> {
        // Create base directory if it doesn't exist
        std::fs::create_dir_all(&base_path)?;

        let manager = Self {
            collections: DashMap::new(),
            aliases: DashMap::new(),
            base_path,
        };

        // Load existing collections
        manager.load_collections()?;

        Ok(manager)
    }

    /// Create a new collection
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name (must be unique)
    /// * `config` - Collection configuration
    ///
    /// # Errors
    ///
    /// Returns `CollectionAlreadyExists` if a collection with the same name exists
    pub fn create_collection(&self, name: &str, config: CollectionConfig) -> Result<()> {
        // Validate collection name
        Self::validate_name(name)?;

        // Check if collection already exists
        if self.collections.contains_key(name) {
            return Err(CollectionError::CollectionAlreadyExists {
                name: name.to_string(),
            });
        }

        // Check if an alias with this name exists
        if self.aliases.contains_key(name) {
            return Err(CollectionError::InvalidName {
                name: name.to_string(),
                reason: "An alias with this name already exists".to_string(),
            });
        }

        // Create storage path for this collection
        let storage_path = self.base_path.join(name);
        std::fs::create_dir_all(&storage_path)?;

        let db_path = storage_path
            .join("vectors.db")
            .to_string_lossy()
            .to_string();

        // Create collection
        let collection = Collection::new(name.to_string(), config, db_path)?;

        // Save metadata
        self.save_collection_metadata(&collection)?;

        // Add to collections map
        self.collections
            .insert(name.to_string(), Arc::new(RwLock::new(collection)));

        Ok(())
    }

    /// Delete a collection
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name to delete
    ///
    /// # Errors
    ///
    /// Returns `CollectionNotFound` if collection doesn't exist
    /// Returns `CollectionHasAliases` if collection has active aliases
    pub fn delete_collection(&self, name: &str) -> Result<()> {
        // Check if collection exists
        if !self.collections.contains_key(name) {
            return Err(CollectionError::CollectionNotFound {
                name: name.to_string(),
            });
        }

        // Check for active aliases
        let active_aliases: Vec<String> = self
            .aliases
            .iter()
            .filter(|entry| entry.value() == name)
            .map(|entry| entry.key().clone())
            .collect();

        if !active_aliases.is_empty() {
            return Err(CollectionError::CollectionHasAliases {
                collection: name.to_string(),
                aliases: active_aliases,
            });
        }

        // Remove from collections map
        self.collections.remove(name);

        // Delete from disk
        let collection_path = self.base_path.join(name);
        if collection_path.exists() {
            std::fs::remove_dir_all(&collection_path)?;
        }

        Ok(())
    }

    /// Get a collection by name or alias
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name or alias
    pub fn get_collection(&self, name: &str) -> Option<Arc<RwLock<Collection>>> {
        // Try to resolve as alias first
        let collection_name = self.resolve_alias(name).unwrap_or_else(|| name.to_string());

        self.collections
            .get(&collection_name)
            .map(|entry| entry.value().clone())
    }

    /// List all collection names
    pub fn list_collections(&self) -> Vec<String> {
        self.collections
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Check if a collection exists
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name (not alias)
    pub fn collection_exists(&self, name: &str) -> bool {
        self.collections.contains_key(name)
    }

    /// Get statistics for a collection
    pub fn collection_stats(&self, name: &str) -> Result<CollectionStats> {
        let collection =
            self.get_collection(name)
                .ok_or_else(|| CollectionError::CollectionNotFound {
                    name: name.to_string(),
                })?;

        let guard = collection.read();
        guard.stats()
    }

    // ===== Alias Management =====

    /// Create an alias for a collection
    ///
    /// # Arguments
    ///
    /// * `alias` - Alias name (must be unique)
    /// * `collection` - Target collection name
    ///
    /// # Errors
    ///
    /// Returns `AliasAlreadyExists` if alias already exists
    /// Returns `CollectionNotFound` if target collection doesn't exist
    pub fn create_alias(&self, alias: &str, collection: &str) -> Result<()> {
        // Validate alias name
        Self::validate_name(alias)?;

        // Check if alias already exists
        if self.aliases.contains_key(alias) {
            return Err(CollectionError::AliasAlreadyExists {
                alias: alias.to_string(),
            });
        }

        // Check if a collection with this name exists
        if self.collections.contains_key(alias) {
            return Err(CollectionError::InvalidName {
                name: alias.to_string(),
                reason: "A collection with this name already exists".to_string(),
            });
        }

        // Verify target collection exists
        if !self.collections.contains_key(collection) {
            return Err(CollectionError::CollectionNotFound {
                name: collection.to_string(),
            });
        }

        // Create alias
        self.aliases
            .insert(alias.to_string(), collection.to_string());

        // Save aliases
        self.save_aliases()?;

        Ok(())
    }

    /// Delete an alias
    ///
    /// # Arguments
    ///
    /// * `alias` - Alias name to delete
    ///
    /// # Errors
    ///
    /// Returns `AliasNotFound` if alias doesn't exist
    pub fn delete_alias(&self, alias: &str) -> Result<()> {
        if self.aliases.remove(alias).is_none() {
            return Err(CollectionError::AliasNotFound {
                alias: alias.to_string(),
            });
        }

        // Save aliases
        self.save_aliases()?;

        Ok(())
    }

    /// Switch an alias to point to a different collection
    ///
    /// # Arguments
    ///
    /// * `alias` - Alias name
    /// * `new_collection` - New target collection name
    ///
    /// # Errors
    ///
    /// Returns `AliasNotFound` if alias doesn't exist
    /// Returns `CollectionNotFound` if new collection doesn't exist
    pub fn switch_alias(&self, alias: &str, new_collection: &str) -> Result<()> {
        // Verify alias exists
        if !self.aliases.contains_key(alias) {
            return Err(CollectionError::AliasNotFound {
                alias: alias.to_string(),
            });
        }

        // Verify new collection exists
        if !self.collections.contains_key(new_collection) {
            return Err(CollectionError::CollectionNotFound {
                name: new_collection.to_string(),
            });
        }

        // Update alias
        self.aliases
            .insert(alias.to_string(), new_collection.to_string());

        // Save aliases
        self.save_aliases()?;

        Ok(())
    }

    /// Resolve an alias to a collection name
    ///
    /// # Arguments
    ///
    /// * `name_or_alias` - Collection name or alias
    ///
    /// # Returns
    ///
    /// `Some(collection_name)` if it's an alias, `None` if it's not an alias
    pub fn resolve_alias(&self, name_or_alias: &str) -> Option<String> {
        self.aliases
            .get(name_or_alias)
            .map(|entry| entry.value().clone())
    }

    /// List all aliases with their target collections
    pub fn list_aliases(&self) -> Vec<(String, String)> {
        self.aliases
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Check if a name is an alias
    pub fn is_alias(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    // ===== Internal Methods =====

    /// Validate a collection or alias name
    fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(CollectionError::InvalidName {
                name: name.to_string(),
                reason: "Name cannot be empty".to_string(),
            });
        }

        if name.len() > 255 {
            return Err(CollectionError::InvalidName {
                name: name.to_string(),
                reason: "Name too long (max 255 characters)".to_string(),
            });
        }

        // Only allow alphanumeric, hyphens, underscores
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(CollectionError::InvalidName {
                name: name.to_string(),
                reason: "Name can only contain letters, numbers, hyphens, and underscores"
                    .to_string(),
            });
        }

        Ok(())
    }

    /// Load existing collections from disk
    fn load_collections(&self) -> Result<()> {
        if !self.base_path.exists() {
            return Ok(());
        }

        // Load aliases
        self.load_aliases()?;

        // Scan for collection directories
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Skip special directories
                if name.starts_with('.') || name == "aliases.json" {
                    continue;
                }

                // Try to load collection metadata
                if let Ok(metadata) = self.load_collection_metadata(&name) {
                    let db_path = path.join("vectors.db").to_string_lossy().to_string();

                    // Recreate collection
                    if let Ok(mut collection) =
                        Collection::new(metadata.name.clone(), metadata.config, db_path)
                    {
                        collection.created_at = metadata.created_at;
                        collection.updated_at = metadata.updated_at;

                        self.collections
                            .insert(name.clone(), Arc::new(RwLock::new(collection)));
                    }
                }
            }
        }

        Ok(())
    }

    /// Save collection metadata to disk
    fn save_collection_metadata(&self, collection: &Collection) -> Result<()> {
        let metadata = CollectionMetadata {
            name: collection.name.clone(),
            config: collection.config.clone(),
            created_at: collection.created_at,
            updated_at: collection.updated_at,
        };

        let metadata_path = self.base_path.join(&collection.name).join("metadata.json");

        let json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(metadata_path, json)?;

        Ok(())
    }

    /// Load collection metadata from disk
    fn load_collection_metadata(&self, name: &str) -> Result<CollectionMetadata> {
        let metadata_path = self.base_path.join(name).join("metadata.json");
        let json = std::fs::read_to_string(metadata_path)?;
        let metadata: CollectionMetadata = serde_json::from_str(&json)?;
        Ok(metadata)
    }

    /// Save aliases to disk
    fn save_aliases(&self) -> Result<()> {
        let aliases: HashMap<String, String> = self
            .aliases
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let aliases_path = self.base_path.join("aliases.json");
        let json = serde_json::to_string_pretty(&aliases)?;
        std::fs::write(aliases_path, json)?;

        Ok(())
    }

    /// Load aliases from disk
    fn load_aliases(&self) -> Result<()> {
        let aliases_path = self.base_path.join("aliases.json");

        if !aliases_path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(aliases_path)?;
        let aliases: HashMap<String, String> = serde_json::from_str(&json)?;

        for (alias, collection) in aliases {
            self.aliases.insert(alias, collection);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(CollectionManager::validate_name("valid-name_123").is_ok());
        assert!(CollectionManager::validate_name("").is_err());
        assert!(CollectionManager::validate_name("invalid name").is_err());
        assert!(CollectionManager::validate_name("invalid/name").is_err());
    }

    #[test]
    fn test_collection_manager() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("ruvector_test_collections");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let manager = CollectionManager::new(temp_dir.clone())?;

        // Create collection
        let config = CollectionConfig::with_dimensions(128);
        manager.create_collection("test", config)?;

        assert!(manager.collection_exists("test"));
        assert_eq!(manager.list_collections().len(), 1);

        // Create alias
        manager.create_alias("test_alias", "test")?;
        assert!(manager.is_alias("test_alias"));
        assert_eq!(
            manager.resolve_alias("test_alias"),
            Some("test".to_string())
        );

        // Get collection by alias
        assert!(manager.get_collection("test_alias").is_some());

        // Cleanup
        manager.delete_alias("test_alias")?;
        manager.delete_collection("test")?;
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }
}
