# Ed25519 Cryptographic Verification - Usage Guide

## ✅ REAL IMPLEMENTATION STATUS

As of v1.2.9+, Goalie includes a **REAL Ed25519 cryptographic implementation** using the `@noble/ed25519` library. This replaces the previous mock implementation with actual cryptographic signing and verification capabilities.

## 🔑 Features Implemented

### Working Features ✅
- **Key Pair Generation**: Generate real Ed25519 key pairs
- **Message Signing**: Sign any message with Ed25519 private key
- **Signature Verification**: Verify signatures with public keys
- **Tamper Detection**: Detects if signed data has been modified
- **Certificate Chains**: Create and verify mandate certificates
- **Citation Signing**: Sign research citations for authenticity
- **Batch Verification**: Verify multiple citations at once
- **Performance**: ~3ms per sign+verify operation

### Partially Implemented ⚠️
- **Trusted Issuer Registry**: Framework exists but needs real public keys
- **Automatic Source Verification**: Requires source cooperation

### Not Yet Implemented ❌
- **Automatic Key Distribution**: Manual key setup required
- **Source Integration**: Sources don't actually sign their content yet

## 🚀 Quick Start

### 1. Generate a Key Pair

```javascript
import { generateEd25519KeyPair } from 'goalie';

const keyPair = await generateEd25519KeyPair();
console.log(keyPair.example); // Shows how to save keys
```

### 2. Set Environment Variables

```bash
# Add to your .env file
ED25519_PRIVATE_KEY="your-base64-private-key"
ED25519_PUBLIC_KEY="your-base64-public-key"
```

### 3. Use with CLI

```bash
# Basic search with verification attempt
goalie search "Your query" --verify

# Require signatures (experimental - most sources won't have them)
goalie search "Your query" --verify --strict-verify

# Sign your own research results
goalie search "Your query" \
  --sign \
  --sign-key "$ED25519_PRIVATE_KEY" \
  --key-id "my-research-key"
```

## 📖 Detailed Examples

### Example 1: Sign and Verify a Research Finding

```javascript
import { Ed25519Verifier } from 'goalie';

// Create verifier with your keys
const verifier = new Ed25519Verifier({
  enabled: true,
  privateKey: process.env.ED25519_PRIVATE_KEY,
  publicKey: process.env.ED25519_PUBLIC_KEY,
  keyId: 'researcher-1',
  signResult: true
});

// Sign a research finding
const finding = "Tesla's Q3 revenue grew 35%";
const signature = await verifier.sign(finding);

// Later, verify the finding hasn't been tampered with
const isValid = await verifier.verify(
  finding,
  signature.signature,
  signature.publicKey
);

console.log(`Finding is ${isValid.valid ? 'authentic' : 'TAMPERED'}`);
```

### Example 2: Create a Certificate Chain

```javascript
// Create a certificate for research data
const cert = await verifier.createCertificate(
  'q3-financial-data',     // Subject
  publicKey,                // Public key for this data
  365                       // Valid for 365 days
);

// Verify the certificate chain
const isChainValid = await verifier.verifyCertificateChain(cert.id);
```

### Example 3: Verify Citations in Batch

```javascript
// Sign multiple citations
const citations = [
  { citation: "AI improves by 40%", url: "https://example.com/1" },
  { citation: "Revenue up 35%", url: "https://example.com/2" }
];

// Sign each citation
const signedCitations = await Promise.all(
  citations.map(c => verifier.signCitation(c))
);

// Verify all citations
const result = await verifier.verifySearchResult(signedCitations);
console.log(`Verified: ${result.verified}/${result.total}`);
```

## 🔐 Security Considerations

### What This Provides
- **Cryptographic Signatures**: Real Ed25519 256-bit signatures
- **Tamper Detection**: Any modification invalidates the signature
- **Non-Repudiation**: Signed data can be attributed to key holder
- **Public Verification**: Anyone with public key can verify

### What This Doesn't Provide (Yet)
- **Source Authentication**: Most web sources don't sign their content
- **Trust Network**: No established web of trust for sources
- **Key Management**: You must manage keys yourself
- **Automatic Verification**: Sources must cooperate to enable verification

## 🧪 Testing the Implementation

Run the test suite to verify Ed25519 is working:

```bash
# Build the project
npm run build

# Run Ed25519 tests
node test-real-ed25519.js
```

Expected output:
```
✅ Signature verification: VALID
✅ Tampered message verification: INVALID (CORRECT!)
✅ Completed 100 sign+verify operations in ~300ms
```

## 📊 Performance

- **Key Generation**: ~50ms
- **Signing**: ~1.5ms per signature
- **Verification**: ~1.5ms per verification
- **Total Round Trip**: ~3ms for sign + verify

## 🔧 API Reference

### Ed25519Verifier Class

```typescript
class Ed25519Verifier {
  constructor(config: Ed25519Config);

  // Core operations
  async sign(message: string): Promise<SignatureResult>;
  async verify(message: string, signature: string, publicKey: string): Promise<VerificationResult>;

  // Citation operations
  async signCitation(citation: CitationSignature): Promise<CitationSignature>;
  async verifyCitation(citation: CitationSignature): Promise<VerificationResult>;

  // Certificate operations
  async createCertificate(subject: string, publicKey: string, validDays: number): Promise<MandateCertificate>;
  async verifyCertificateChain(certId: string): Promise<boolean>;

  // Batch operations
  async verifySearchResult(citations: CitationSignature[]): Promise<BatchResult>;
}
```

### Configuration Options

```typescript
interface Ed25519Config {
  enabled: boolean;           // Enable Ed25519 features
  requireSignatures?: boolean; // Require all sources to be signed
  signResult?: boolean;       // Sign your research results
  privateKey?: string;        // Base64 encoded private key
  publicKey?: string;         // Base64 encoded public key
  keyId?: string;            // Identifier for your key
  trustedIssuers?: string[]; // List of trusted domains
}
```

## ⚠️ Important Notes

1. **This is Real Cryptography**: Unlike the previous mock, this uses actual Ed25519 signatures that provide real security.

2. **Limited Source Support**: Most web sources don't provide Ed25519 signatures, so verification will often show "untrusted" even for legitimate sources.

3. **Key Management**: You are responsible for keeping your private key secure. Never commit it to version control.

4. **Experimental Feature**: While the cryptography is real, the integration with web sources is still experimental.

## 🚦 Migration from Mock

If you were using the mock implementation:

1. **Generate Real Keys**: The mock accepted any string; now you need real Ed25519 keys
2. **Update Environment**: Use the generated Base64 keys, not placeholder strings
3. **Expect Different Results**: Real verification will fail for unsigned content
4. **Performance**: Real crypto is slightly slower (~3ms vs instant mock)

## 📚 Further Reading

- [Ed25519 RFC 8032](https://datatracker.ietf.org/doc/html/rfc8032)
- [Noble Cryptography Library](https://github.com/paulmillr/noble-ed25519)
- [Digital Signatures Explained](https://en.wikipedia.org/wiki/Digital_signature)

## 🤝 Contributing

To improve Ed25519 integration:
1. Add real public keys for trusted sources
2. Implement key exchange protocols
3. Create browser extension for automatic verification
4. Work with sources to sign their content

---

**Note**: This is a real cryptographic implementation. The signatures are genuine Ed25519 signatures that provide actual security guarantees, unlike the previous mock implementation.