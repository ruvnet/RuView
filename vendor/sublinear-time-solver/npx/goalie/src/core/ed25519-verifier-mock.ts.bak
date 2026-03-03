/**
 * Ed25519 Cryptographic Verification for Anti-Hallucination
 *
 * Implements mandate certificates and signature verification
 * to ensure AI content authenticity and prevent hallucination
 * in the GOAP system.
 */

import crypto from 'crypto';

export interface Ed25519KeyPair {
  publicKey: string;  // Base64 encoded
  privateKey: string; // Base64 encoded
  keyId?: string;     // Optional key identifier
}

export interface MandateCertificate {
  version: '1.0';
  issuer: string;
  subject: string;
  publicKey: string;
  validFrom: string;
  validUntil: string;
  certId: string;
  parentCertId?: string;
  signature: string;  // Ed25519 signature of the certificate
}

export interface SignedContent {
  content: any;
  metadata: {
    timestamp: number;
    nonce: string;
    algorithm: 'Ed25519';
    keyId: string;
    certId?: string;
  };
  signature: string;
}

export interface VerificationResult {
  valid: boolean;
  issuer?: string;
  certChainValid?: boolean;
  timestamp?: number;
  errors?: string[];
}

export class Ed25519Verifier {
  private trustedCerts: Map<string, MandateCertificate> = new Map();
  private publicKeys: Map<string, string> = new Map();
  private readonly TIMESTAMP_WINDOW_MS = 5 * 60 * 1000; // 5 minutes

  /**
   * Generate a new Ed25519 key pair
   */
  generateKeyPair(): Ed25519KeyPair {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');

    return {
      publicKey: publicKey.export({ type: 'spki', format: 'der' }).toString('base64'),
      privateKey: privateKey.export({ type: 'pkcs8', format: 'der' }).toString('base64'),
      keyId: crypto.randomBytes(16).toString('hex')
    };
  }

  /**
   * Sign content with Ed25519 private key
   */
  signContent(
    content: any,
    privateKey: string,
    keyId: string,
    certId?: string
  ): SignedContent {
    const timestamp = Date.now();
    const nonce = crypto.randomBytes(16).toString('hex');

    // Create canonical message for signing
    const message = this.createCanonicalMessage(content, timestamp, nonce);

    // Import private key
    const key = crypto.createPrivateKey({
      key: Buffer.from(privateKey, 'base64'),
      format: 'der',
      type: 'pkcs8'
    });

    // Sign the message
    const signature = crypto.sign(null, Buffer.from(message), key).toString('base64');

    return {
      content,
      metadata: {
        timestamp,
        nonce,
        algorithm: 'Ed25519',
        keyId,
        certId
      },
      signature
    };
  }

  /**
   * Verify signed content
   */
  verifyContent(signedContent: SignedContent): VerificationResult {
    const errors: string[] = [];

    try {
      // Check timestamp freshness
      const now = Date.now();
      const { timestamp } = signedContent.metadata;

      if (Math.abs(now - timestamp) > this.TIMESTAMP_WINDOW_MS) {
        errors.push('Timestamp outside acceptable window');
      }

      // Get public key
      const publicKey = this.getPublicKey(signedContent.metadata.keyId, signedContent.metadata.certId);

      if (!publicKey) {
        return {
          valid: false,
          errors: ['Public key not found']
        };
      }

      // Recreate canonical message
      const message = this.createCanonicalMessage(
        signedContent.content,
        signedContent.metadata.timestamp,
        signedContent.metadata.nonce
      );

      // Import public key
      const key = crypto.createPublicKey({
        key: Buffer.from(publicKey, 'base64'),
        format: 'der',
        type: 'spki'
      });

      // Verify signature
      const valid = crypto.verify(
        null,
        Buffer.from(message),
        key,
        Buffer.from(signedContent.signature, 'base64')
      );

      // Verify certificate chain if present
      let certChainValid = true;
      let issuer: string | undefined;

      if (signedContent.metadata.certId) {
        const cert = this.trustedCerts.get(signedContent.metadata.certId);
        if (cert) {
          certChainValid = this.verifyCertificateChain(cert);
          issuer = cert.issuer;
        } else {
          errors.push('Certificate not found');
          certChainValid = false;
        }
      }

      return {
        valid: valid && errors.length === 0,
        issuer,
        certChainValid,
        timestamp,
        errors: errors.length > 0 ? errors : undefined
      };

    } catch (error) {
      return {
        valid: false,
        errors: [`Verification failed: ${error instanceof Error ? error.message : String(error)}`]
      };
    }
  }

  /**
   * Register a mandate certificate
   */
  registerCertificate(cert: MandateCertificate): boolean {
    // Verify certificate signature
    if (!this.verifyCertificateSignature(cert)) {
      return false;
    }

    // Check validity period
    const now = new Date();
    const validFrom = new Date(cert.validFrom);
    const validUntil = new Date(cert.validUntil);

    if (now < validFrom || now > validUntil) {
      return false;
    }

    // Store certificate
    this.trustedCerts.set(cert.certId, cert);
    this.publicKeys.set(cert.subject, cert.publicKey);

    return true;
  }

  /**
   * Create a mandate certificate
   */
  createCertificate(
    subject: string,
    publicKey: string,
    issuerPrivateKey: string,
    issuer: string,
    parentCertId?: string
  ): MandateCertificate {
    const validFrom = new Date().toISOString();
    const validUntil = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(); // 1 year
    const certId = crypto.randomBytes(16).toString('hex');

    const certData = {
      version: '1.0' as const,
      issuer,
      subject,
      publicKey,
      validFrom,
      validUntil,
      certId,
      parentCertId
    };

    // Sign certificate
    const message = this.canonicalizeCertificate(certData);
    const key = crypto.createPrivateKey({
      key: Buffer.from(issuerPrivateKey, 'base64'),
      format: 'der',
      type: 'pkcs8'
    });

    const signature = crypto.sign(null, Buffer.from(message), key).toString('base64');

    return {
      ...certData,
      signature
    };
  }

  /**
   * Create canonical message for signing
   */
  private createCanonicalMessage(content: any, timestamp: number, nonce: string): string {
    // Use JSON Canonicalization Scheme (JCS) - simplified version
    const canonical = {
      content: this.canonicalizeJSON(content),
      timestamp,
      nonce
    };

    return JSON.stringify(canonical);
  }

  /**
   * Canonicalize JSON for consistent hashing
   */
  private canonicalizeJSON(obj: any): any {
    if (obj === null || typeof obj !== 'object') {
      return obj;
    }

    if (Array.isArray(obj)) {
      return obj.map(item => this.canonicalizeJSON(item));
    }

    const sorted: any = {};
    Object.keys(obj).sort().forEach(key => {
      sorted[key] = this.canonicalizeJSON(obj[key]);
    });

    return sorted;
  }

  /**
   * Canonicalize certificate for signing
   */
  private canonicalizeCertificate(cert: Omit<MandateCertificate, 'signature'>): string {
    return JSON.stringify({
      version: cert.version,
      issuer: cert.issuer,
      subject: cert.subject,
      publicKey: cert.publicKey,
      validFrom: cert.validFrom,
      validUntil: cert.validUntil,
      certId: cert.certId,
      parentCertId: cert.parentCertId
    });
  }

  /**
   * Verify certificate signature
   */
  private verifyCertificateSignature(cert: MandateCertificate): boolean {
    try {
      // Get issuer's public key
      const issuerKey = this.publicKeys.get(cert.issuer);
      if (!issuerKey) {
        return false; // Unknown issuer
      }

      const certData = { ...cert };
      delete (certData as any).signature;

      const message = this.canonicalizeCertificate(certData);
      const key = crypto.createPublicKey({
        key: Buffer.from(issuerKey, 'base64'),
        format: 'der',
        type: 'spki'
      });

      return crypto.verify(
        null,
        Buffer.from(message),
        key,
        Buffer.from(cert.signature, 'base64')
      );
    } catch {
      return false;
    }
  }

  /**
   * Verify certificate chain
   */
  private verifyCertificateChain(cert: MandateCertificate): boolean {
    let current = cert;
    const visited = new Set<string>();

    while (current.parentCertId) {
      if (visited.has(current.certId)) {
        return false; // Circular chain
      }
      visited.add(current.certId);

      const parent = this.trustedCerts.get(current.parentCertId);
      if (!parent) {
        return false; // Broken chain
      }

      if (!this.verifyCertificateSignature(current)) {
        return false; // Invalid signature
      }

      current = parent;
    }

    return true;
  }

  /**
   * Get public key for verification
   */
  private getPublicKey(keyId: string, certId?: string): string | null {
    if (certId) {
      const cert = this.trustedCerts.get(certId);
      if (cert) {
        return cert.publicKey;
      }
    }

    return this.publicKeys.get(keyId) || null;
  }

  /**
   * Register a trusted root key (for bootstrapping trust)
   */
  registerTrustedKey(keyId: string, publicKey: string): void {
    this.publicKeys.set(keyId, publicKey);
  }

  /**
   * Export trusted certificates for persistence
   */
  exportCertificates(): MandateCertificate[] {
    return Array.from(this.trustedCerts.values());
  }

  /**
   * Import trusted certificates
   */
  importCertificates(certs: MandateCertificate[]): void {
    certs.forEach(cert => this.registerCertificate(cert));
  }
}

/**
 * Integration with GOAP anti-hallucination
 */
export class AntiHallucinationVerifier {
  private verifier: Ed25519Verifier;

  constructor(verifier: Ed25519Verifier) {
    this.verifier = verifier;
  }

  /**
   * Verify that citations are signed by trusted sources
   */
  verifyCitations(citations: any[], requireSignatures: boolean = false): {
    verified: number;
    total: number;
    untrusted: string[];
  } {
    let verified = 0;
    const untrusted: string[] = [];

    citations.forEach(citation => {
      if (citation.signature) {
        const result = this.verifier.verifyContent(citation);
        if (result.valid && result.certChainValid) {
          verified++;
        } else {
          untrusted.push(citation.url || citation.title || 'Unknown');
        }
      } else if (!requireSignatures) {
        // Count unsigned citations as verified if signatures not required
        verified++;
      } else {
        untrusted.push(citation.url || citation.title || 'Unknown');
      }
    });

    return {
      verified,
      total: citations.length,
      untrusted
    };
  }

  /**
   * Sign search results with Ed25519
   */
  signSearchResult(
    result: any,
    privateKey: string,
    keyId: string,
    certId?: string
  ): SignedContent {
    return this.verifier.signContent(result, privateKey, keyId, certId);
  }

  /**
   * Verify signed search results
   */
  verifySearchResult(signedResult: SignedContent): VerificationResult {
    return this.verifier.verifyContent(signedResult);
  }
}

// Export for use in GOAP tools
export default Ed25519Verifier;