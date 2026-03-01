//! Federation handshake protocol
//!
//! Implements the cryptographic handshake for joining a federation:
//! 1. Post-quantum key exchange
//! 2. Channel establishment
//! 3. Capability negotiation

use crate::{
    crypto::{EncryptedChannel, PostQuantumKeypair},
    FederationError, PeerAddress, Result,
};
use serde::{Deserialize, Serialize};

/// Capabilities supported by a federation node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Capability name
    pub name: String,
    /// Capability version
    pub version: String,
    /// Additional parameters
    pub params: std::collections::HashMap<String, String>,
}

impl Capability {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            params: std::collections::HashMap::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

/// Token granting access to a federation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationToken {
    /// Peer identifier
    pub peer_id: String,
    /// Negotiated capabilities
    pub capabilities: Vec<Capability>,
    /// Token expiry timestamp
    pub expires: u64,
    /// Channel secret (not serialized)
    #[serde(skip)]
    pub(crate) channel: Option<EncryptedChannel>,
}

impl FederationToken {
    /// Check if token is still valid
    pub fn is_valid(&self) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now < self.expires
    }

    /// Get the encrypted channel
    pub fn channel(&self) -> Option<&EncryptedChannel> {
        self.channel.as_ref()
    }
}

/// Join a federation by performing cryptographic handshake
///
/// # Protocol
///
/// 1. Generate ephemeral keypair
/// 2. Send public key to peer
/// 3. Receive encapsulated shared secret
/// 4. Establish encrypted channel
/// 5. Exchange and negotiate capabilities
///
/// # Implementation from PSEUDOCODE.md
///
/// ```pseudocode
/// FUNCTION JoinFederation(local_node, peer_address):
///     (local_public, local_secret) = Kyber.KeyGen()
///     SendMessage(peer_address, FederationRequest(local_public))
///     response = ReceiveMessage(peer_address)
///     shared_secret = Kyber.Decapsulate(response.ciphertext, local_secret)
///     (encrypt_key, mac_key) = DeriveKeys(shared_secret)
///     channel = EncryptedChannel(peer_address, encrypt_key, mac_key)
///     local_caps = local_node.capabilities()
///     peer_caps = channel.exchange(local_caps)
///     terms = NegotiateFederationTerms(local_caps, peer_caps)
///     token = FederationToken(...)
///     RETURN token
/// ```
pub async fn join_federation(
    _local_keys: &PostQuantumKeypair,
    peer: &PeerAddress,
) -> Result<FederationToken> {
    // Step 1: Post-quantum key exchange
    let (shared_secret, _ciphertext) = PostQuantumKeypair::encapsulate(&peer.public_key)?;

    // Step 2: Establish encrypted channel
    // In real implementation, we would:
    // - Send our public key to peer
    // - Receive peer's ciphertext
    // - Decapsulate to get shared secret
    // For now, we simulate both sides
    let peer_id = generate_peer_id(&peer.host, peer.port);
    let channel = EncryptedChannel::new(peer_id.clone(), shared_secret);

    // Step 3: Exchange capabilities
    let local_capabilities = get_local_capabilities();

    // In real implementation:
    // let peer_capabilities = channel.send_and_receive(local_capabilities).await?;
    let peer_capabilities = simulate_peer_capabilities();

    // Step 4: Negotiate federation terms
    let capabilities = negotiate_capabilities(local_capabilities, peer_capabilities)?;

    // Step 5: Create federation token
    let token = FederationToken {
        peer_id,
        capabilities,
        expires: current_timestamp() + TOKEN_VALIDITY_SECONDS,
        channel: Some(channel),
    };

    Ok(token)
}

/// Get capabilities supported by this node
fn get_local_capabilities() -> Vec<Capability> {
    vec![
        Capability::new("query", "1.0").with_param("max_results", "1000"),
        Capability::new("consensus", "1.0").with_param("algorithm", "pbft"),
        Capability::new("crdt", "1.0").with_param("types", "gset,lww"),
        Capability::new("onion", "1.0").with_param("max_hops", "5"),
    ]
}

/// Simulate peer capabilities (placeholder)
fn simulate_peer_capabilities() -> Vec<Capability> {
    vec![
        Capability::new("query", "1.0").with_param("max_results", "500"),
        Capability::new("consensus", "1.0").with_param("algorithm", "pbft"),
        Capability::new("crdt", "1.0").with_param("types", "gset,lww,orset"),
    ]
}

/// Negotiate capabilities between local and peer
fn negotiate_capabilities(
    local: Vec<Capability>,
    peer: Vec<Capability>,
) -> Result<Vec<Capability>> {
    let mut negotiated = Vec::new();

    // Find intersection of capabilities
    for local_cap in &local {
        if let Some(peer_cap) = peer.iter().find(|p| p.name == local_cap.name) {
            // Check version compatibility
            if is_compatible(&local_cap.version, &peer_cap.version) {
                // Take minimum of parameters
                let mut merged = local_cap.clone();

                for (key, local_val) in &local_cap.params {
                    if let Some(peer_val) = peer_cap.params.get(key) {
                        // Take minimum value (more conservative)
                        if let (Ok(local_num), Ok(peer_num)) =
                            (local_val.parse::<u64>(), peer_val.parse::<u64>())
                        {
                            merged
                                .params
                                .insert(key.clone(), local_num.min(peer_num).to_string());
                        }
                    }
                }

                negotiated.push(merged);
            }
        }
    }

    if negotiated.is_empty() {
        return Err(FederationError::ConsensusError(
            "No compatible capabilities".to_string(),
        ));
    }

    Ok(negotiated)
}

/// Check if two versions are compatible
fn is_compatible(v1: &str, v2: &str) -> bool {
    // Simple major version check
    let major1 = v1.split('.').next().unwrap_or("0");
    let major2 = v2.split('.').next().unwrap_or("0");
    major1 == major2
}

/// Generate a peer ID from address
fn generate_peer_id(host: &str, port: u16) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(host.as_bytes());
    hasher.update(&port.to_le_bytes());
    hex::encode(&hasher.finalize()[..16])
}

/// Get current timestamp in seconds
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Token validity period (1 hour)
const TOKEN_VALIDITY_SECONDS: u64 = 3600;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_join_federation() {
        let local_keys = PostQuantumKeypair::generate();
        let peer_keys = PostQuantumKeypair::generate();

        let peer = PeerAddress::new(
            "localhost".to_string(),
            8080,
            peer_keys.public_key().to_vec(),
        );

        let token = join_federation(&local_keys, &peer).await.unwrap();

        assert!(token.is_valid());
        assert!(!token.capabilities.is_empty());
        assert!(token.channel.is_some());
    }

    #[test]
    fn test_capability_negotiation() {
        let local = vec![Capability::new("test", "1.0").with_param("limit", "100")];

        let peer = vec![Capability::new("test", "1.0").with_param("limit", "50")];

        let result = negotiate_capabilities(local, peer).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].params.get("limit").unwrap(), "50");
    }

    #[test]
    fn test_version_compatibility() {
        assert!(is_compatible("1.0", "1.1"));
        assert!(is_compatible("1.5", "1.0"));
        assert!(!is_compatible("1.0", "2.0"));
        assert!(!is_compatible("2.1", "1.9"));
    }
}
