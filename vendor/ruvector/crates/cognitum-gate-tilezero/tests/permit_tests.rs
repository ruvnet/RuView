//! Comprehensive tests for permit token signing and verification
//!
//! Tests cover:
//! - Token creation and signing
//! - Signature verification
//! - TTL validation
//! - Security tests (invalid signatures, replay attacks, tamper detection)

use cognitum_gate_tilezero::permit::{
    PermitState, PermitToken, TokenDecodeError, Verifier, VerifyError,
};
use cognitum_gate_tilezero::GateDecision;

fn create_test_token(action_id: &str, sequence: u64) -> PermitToken {
    PermitToken {
        decision: GateDecision::Permit,
        action_id: action_id.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
        ttl_ns: 60_000_000_000, // 60 seconds
        witness_hash: [0u8; 32],
        sequence,
        signature: [0u8; 64],
    }
}

#[cfg(test)]
mod token_creation {
    use super::*;

    #[test]
    fn test_token_fields() {
        let token = create_test_token("test-action", 42);

        assert_eq!(token.action_id, "test-action");
        assert_eq!(token.sequence, 42);
        assert_eq!(token.decision, GateDecision::Permit);
        assert!(token.timestamp > 0);
        assert_eq!(token.ttl_ns, 60_000_000_000);
    }

    #[test]
    fn test_token_with_different_decisions() {
        let permit_token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let defer_token = PermitToken {
            decision: GateDecision::Defer,
            ..permit_token.clone()
        };

        let deny_token = PermitToken {
            decision: GateDecision::Deny,
            ..permit_token.clone()
        };

        assert_eq!(permit_token.decision, GateDecision::Permit);
        assert_eq!(defer_token.decision, GateDecision::Defer);
        assert_eq!(deny_token.decision, GateDecision::Deny);
    }
}

#[cfg(test)]
mod ttl_validation {
    use super::*;

    #[test]
    fn test_token_valid_within_ttl() {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: now_ns,
            ttl_ns: 60_000_000_000, // 60 seconds
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        // Check immediately - should be valid
        assert!(token.is_valid_time(now_ns));

        // Check 30 seconds later - still valid
        assert!(token.is_valid_time(now_ns + 30_000_000_000));
    }

    #[test]
    fn test_token_invalid_after_ttl() {
        let timestamp = 1000000000u64;
        let ttl = 60_000_000_000u64; // 60 seconds

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp,
            ttl_ns: ttl,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        // After TTL expires
        let after_expiry = timestamp + ttl + 1;
        assert!(!token.is_valid_time(after_expiry));
    }

    #[test]
    fn test_token_valid_at_exactly_expiry() {
        let timestamp = 1000000000u64;
        let ttl = 60_000_000_000u64;

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp,
            ttl_ns: ttl,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        // Exactly at expiry boundary
        let at_expiry = timestamp + ttl;
        assert!(token.is_valid_time(at_expiry));
    }

    #[test]
    fn test_zero_ttl() {
        let timestamp = 1000000000u64;

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp,
            ttl_ns: 0, // Immediate expiry
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        // Valid at exact timestamp
        assert!(token.is_valid_time(timestamp));

        // Invalid one nanosecond later
        assert!(!token.is_valid_time(timestamp + 1));
    }
}

#[cfg(test)]
mod signing {
    use super::*;

    #[test]
    fn test_permit_state_creation() {
        let state = PermitState::new();
        // Should be able to get a verifier
        let _verifier = state.verifier();
    }

    #[test]
    fn test_sign_token() {
        let state = PermitState::new();
        let token = create_test_token("test-action", 0);

        let signed = state.sign_token(token);

        // MAC should be set (non-zero)
        assert_ne!(signed.signature, [0u8; 64]);
    }

    #[test]
    fn test_sign_different_tokens_different_macs() {
        let state = PermitState::new();

        let token1 = create_test_token("action-1", 0);
        let token2 = create_test_token("action-2", 1);

        let signed1 = state.sign_token(token1);
        let signed2 = state.sign_token(token2);

        assert_ne!(signed1.signature, signed2.signature);
    }

    #[test]
    fn test_sign_deterministic() {
        let state = PermitState::new();

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed1 = state.sign_token(token.clone());
        let signed2 = state.sign_token(token);

        // Same input, same key, same output
        assert_eq!(signed1.signature, signed2.signature);
    }

    #[test]
    fn test_sequence_incrementing() {
        let state = PermitState::new();

        let seq1 = state.next_sequence();
        let seq2 = state.next_sequence();
        let seq3 = state.next_sequence();

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        assert_eq!(seq3, 2);
    }
}

#[cfg(test)]
mod verification {
    use super::*;

    #[test]
    fn test_verify_signed_token() {
        let state = PermitState::new();
        let verifier = state.verifier();

        let token = create_test_token("test-action", 0);
        let signed = state.sign_token(token);

        assert!(verifier.verify(&signed).is_ok());
    }

    #[test]
    fn test_verify_unsigned_token_fails() {
        let state = PermitState::new();
        let verifier = state.verifier();

        let token = create_test_token("test-action", 0);
        // Token is not signed (signature is zero)

        // Verification of unsigned token should FAIL
        let result = verifier.verify(&token);
        assert!(result.is_err(), "Unsigned token should fail verification");
    }

    #[test]
    fn test_verify_full_checks_ttl() {
        let state = PermitState::new();
        let verifier = state.verifier();

        // Create an already-expired token
        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1, // Very old
            ttl_ns: 1,    // Very short
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed = state.sign_token(token);

        // Full verification should fail due to expiry
        let result = verifier.verify_full(&signed);
        assert!(matches!(result, Err(VerifyError::Expired)));
    }
}

#[cfg(test)]
mod signable_content {
    use super::*;

    #[test]
    fn test_signable_content_deterministic() {
        let token = create_test_token("test", 42);

        let content1 = token.signable_content();
        let content2 = token.signable_content();

        assert_eq!(content1, content2);
    }

    #[test]
    fn test_signable_content_changes_with_fields() {
        let token1 = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let mut token2 = token1.clone();
        token2.sequence = 1;

        assert_ne!(token1.signable_content(), token2.signable_content());
    }

    #[test]
    fn test_signable_content_excludes_mac() {
        let mut token1 = create_test_token("test", 0);
        let mut token2 = token1.clone();

        token1.signature = [1u8; 64];
        token2.signature = [2u8; 64];

        // Different MACs but same signable content
        assert_eq!(token1.signable_content(), token2.signable_content());
    }

    #[test]
    fn test_signable_content_includes_decision() {
        let token_permit = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let token_deny = PermitToken {
            decision: GateDecision::Deny,
            ..token_permit.clone()
        };

        assert_ne!(
            token_permit.signable_content(),
            token_deny.signable_content()
        );
    }
}

#[cfg(test)]
mod base64_encoding {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let token = create_test_token("test-action", 42);

        let encoded = token.encode_base64();
        let decoded = PermitToken::decode_base64(&encoded).unwrap();

        assert_eq!(token.action_id, decoded.action_id);
        assert_eq!(token.sequence, decoded.sequence);
        assert_eq!(token.decision, decoded.decision);
    }

    #[test]
    fn test_decode_invalid_base64() {
        let result = PermitToken::decode_base64("not valid base64!!!");
        assert!(matches!(result, Err(TokenDecodeError::InvalidBase64)));
    }

    #[test]
    fn test_decode_invalid_json() {
        // Valid base64 but not JSON
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"not json");
        let result = PermitToken::decode_base64(&encoded);
        assert!(matches!(result, Err(TokenDecodeError::InvalidJson)));
    }

    #[test]
    fn test_signed_token_encode_decode() {
        let state = PermitState::new();
        let token = create_test_token("test", 0);
        let signed = state.sign_token(token);

        let encoded = signed.encode_base64();
        let decoded = PermitToken::decode_base64(&encoded).unwrap();

        // MAC should be preserved
        assert_eq!(signed.signature, decoded.signature);
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    /// Test that different keys produce different signatures
    #[test]
    fn test_different_keys_different_signatures() {
        let state1 = PermitState::new();
        let state2 = PermitState::new();

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed1 = state1.sign_token(token.clone());
        let signed2 = state2.sign_token(token);

        assert_ne!(signed1.signature, signed2.signature);
    }

    /// Test cross-key verification fails
    #[test]
    fn test_cross_key_verification_fails() {
        let state1 = PermitState::new();
        let state2 = PermitState::new();
        let verifier2 = state2.verifier();

        let token = create_test_token("test", 0);
        let signed = state1.sign_token(token);

        // Verification with wrong key should FAIL
        let result = verifier2.verify(&signed);
        assert!(result.is_err(), "Cross-key verification should fail");
    }

    /// Test token tampering detection
    #[test]
    fn test_tamper_detection() {
        let state = PermitState::new();
        let verifier = state.verifier();

        let token = create_test_token("test", 0);
        let mut signed = state.sign_token(token);

        // Verify original is valid
        assert!(verifier.verify(&signed).is_ok(), "Original should verify");

        // Tamper with the action_id
        signed.action_id = "tampered".to_string();

        // Verification should now FAIL because signature doesn't match
        let result = verifier.verify(&signed);
        assert!(result.is_err(), "Tampered token should fail verification");
    }

    /// Test replay attack scenario
    #[test]
    fn test_sequence_prevents_replay() {
        let state = PermitState::new();

        let token1 = create_test_token("test", state.next_sequence());
        let token2 = create_test_token("test", state.next_sequence());

        let signed1 = state.sign_token(token1);
        let signed2 = state.sign_token(token2);

        // Different sequences even for same action
        assert_ne!(signed1.sequence, signed2.sequence);
        assert_ne!(signed1.signature, signed2.signature);
    }

    /// Test witness hash binding
    #[test]
    fn test_witness_hash_binding() {
        let state = PermitState::new();

        let mut token1 = create_test_token("test", 0);
        token1.witness_hash = [1u8; 32];

        let mut token2 = create_test_token("test", 0);
        token2.witness_hash = [2u8; 32];

        let signed1 = state.sign_token(token1);
        let signed2 = state.sign_token(token2);

        // Different witness hashes produce different signatures
        assert_ne!(signed1.signature, signed2.signature);
    }
}

#[cfg(test)]
mod custom_key {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_with_custom_key() {
        let custom_key = SigningKey::generate(&mut OsRng);
        let state = PermitState::with_key(custom_key);

        let token = create_test_token("test", 0);
        let signed = state.sign_token(token);

        let verifier = state.verifier();
        assert!(verifier.verify(&signed).is_ok());
    }

    #[test]
    fn test_same_key_same_signatures() {
        let key_bytes: [u8; 32] = [42u8; 32];
        let key1 = SigningKey::from_bytes(&key_bytes);
        let key2 = SigningKey::from_bytes(&key_bytes);

        let state1 = PermitState::with_key(key1);
        let state2 = PermitState::with_key(key2);

        let token = PermitToken {
            decision: GateDecision::Permit,
            action_id: "test".to_string(),
            timestamp: 1000000000,
            ttl_ns: 60000,
            witness_hash: [0u8; 32],
            sequence: 0,
            signature: [0u8; 64],
        };

        let signed1 = state1.sign_token(token.clone());
        let signed2 = state2.sign_token(token);

        assert_eq!(signed1.signature, signed2.signature);
    }
}

// Property-based tests
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_encode_decode_roundtrip(
            action_id in "[a-z]{1,20}",
            sequence in 0u64..1000,
            ttl in 1u64..1000000000
        ) {
            let token = PermitToken {
                decision: GateDecision::Permit,
                action_id,
                timestamp: 1000000000,
                ttl_ns: ttl,
                witness_hash: [0u8; 32],
                sequence,
                signature: [0u8; 64],
            };

            let encoded = token.encode_base64();
            let decoded = PermitToken::decode_base64(&encoded).unwrap();

            assert_eq!(token.action_id, decoded.action_id);
            assert_eq!(token.sequence, decoded.sequence);
        }

        #[test]
        fn prop_ttl_validity(timestamp in 1u64..1000000000000u64, ttl in 1u64..1000000000000u64) {
            let token = PermitToken {
                decision: GateDecision::Permit,
                action_id: "test".to_string(),
                timestamp,
                ttl_ns: ttl,
                witness_hash: [0u8; 32],
                sequence: 0,
                signature: [0u8; 64],
            };

            // Valid at start
            assert!(token.is_valid_time(timestamp));

            // Valid just before expiry
            if ttl > 1 {
                assert!(token.is_valid_time(timestamp + ttl - 1));
            }

            // Invalid after expiry
            assert!(!token.is_valid_time(timestamp + ttl + 1));
        }

        #[test]
        fn prop_signing_adds_mac(action_id in "[a-z]{1,10}") {
            let state = PermitState::new();
            let token = PermitToken {
                decision: GateDecision::Permit,
                action_id,
                timestamp: 1000000000,
                ttl_ns: 60000,
                witness_hash: [0u8; 32],
                sequence: 0,
                signature: [0u8; 64],
            };

            let signed = state.sign_token(token);
            assert_ne!(signed.signature, [0u8; 64]);
        }
    }
}
