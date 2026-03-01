//! QuDAG Identity Management

use super::{
    MlDsa65, MlDsa65PublicKey, MlDsa65SecretKey, MlKem768, MlKem768PublicKey, MlKem768SecretKey,
};

pub struct QuDagIdentity {
    pub node_id: String,
    pub kem_public: MlKem768PublicKey,
    pub kem_secret: MlKem768SecretKey,
    pub dsa_public: MlDsa65PublicKey,
    pub dsa_secret: MlDsa65SecretKey,
}

impl QuDagIdentity {
    pub fn generate() -> Result<Self, IdentityError> {
        let (kem_public, kem_secret) =
            MlKem768::generate_keypair().map_err(|_| IdentityError::KeyGenerationFailed)?;

        let (dsa_public, dsa_secret) =
            MlDsa65::generate_keypair().map_err(|_| IdentityError::KeyGenerationFailed)?;

        // Generate node ID from public key hash
        let node_id = Self::hash_to_id(&kem_public.0[..32]);

        Ok(Self {
            node_id,
            kem_public,
            kem_secret,
            dsa_public,
            dsa_secret,
        })
    }

    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, IdentityError> {
        let sig =
            MlDsa65::sign(&self.dsa_secret, message).map_err(|_| IdentityError::SigningFailed)?;
        Ok(sig.0.to_vec())
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, IdentityError> {
        if signature.len() != super::ml_dsa::ML_DSA_65_SIGNATURE_SIZE {
            return Err(IdentityError::InvalidSignature);
        }

        let mut sig_array = [0u8; super::ml_dsa::ML_DSA_65_SIGNATURE_SIZE];
        sig_array.copy_from_slice(signature);

        MlDsa65::verify(
            &self.dsa_public,
            message,
            &super::ml_dsa::Signature(sig_array),
        )
        .map_err(|_| IdentityError::VerificationFailed)
    }

    pub fn encrypt_for(
        &self,
        recipient_pk: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, IdentityError> {
        if recipient_pk.len() != super::ml_kem::ML_KEM_768_PUBLIC_KEY_SIZE {
            return Err(IdentityError::InvalidPublicKey);
        }

        let mut pk_array = [0u8; super::ml_kem::ML_KEM_768_PUBLIC_KEY_SIZE];
        pk_array.copy_from_slice(recipient_pk);

        let encap = MlKem768::encapsulate(&MlKem768PublicKey(pk_array))
            .map_err(|_| IdentityError::EncryptionFailed)?;

        // Simple XOR encryption with shared secret
        let mut ciphertext = encap.ciphertext.to_vec();
        for (i, byte) in plaintext.iter().enumerate() {
            ciphertext.push(*byte ^ encap.shared_secret[i % 32]);
        }

        Ok(ciphertext)
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, IdentityError> {
        if ciphertext.len() < super::ml_kem::ML_KEM_768_CIPHERTEXT_SIZE {
            return Err(IdentityError::InvalidCiphertext);
        }

        let mut ct_array = [0u8; super::ml_kem::ML_KEM_768_CIPHERTEXT_SIZE];
        ct_array.copy_from_slice(&ciphertext[..super::ml_kem::ML_KEM_768_CIPHERTEXT_SIZE]);

        let shared_secret = MlKem768::decapsulate(&self.kem_secret, &ct_array)
            .map_err(|_| IdentityError::DecryptionFailed)?;

        // Decrypt with XOR
        let encrypted_data = &ciphertext[super::ml_kem::ML_KEM_768_CIPHERTEXT_SIZE..];
        let plaintext: Vec<u8> = encrypted_data
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ shared_secret[i % 32])
            .collect();

        Ok(plaintext)
    }

    fn hash_to_id(data: &[u8]) -> String {
        let hash: u64 = data
            .iter()
            .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));
        format!("qudag_{:016x}", hash)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("Key generation failed")]
    KeyGenerationFailed,
    #[error("Signing failed")]
    SigningFailed,
    #[error("Verification failed")]
    VerificationFailed,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
    #[error("Invalid ciphertext")]
    InvalidCiphertext,
}
