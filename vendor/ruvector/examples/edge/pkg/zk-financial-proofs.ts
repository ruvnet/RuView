/**
 * Zero-Knowledge Financial Proofs
 *
 * Prove financial statements without revealing actual numbers.
 * All proof generation happens in the browser - private data never leaves.
 *
 * @example
 * ```typescript
 * import { ZkFinancialProver, ZkProofVerifier } from './zk-financial-proofs';
 *
 * // Prover (you - with private data)
 * const prover = new ZkFinancialProver();
 * prover.loadIncome([650000, 650000, 680000]); // cents
 * prover.loadBalances([500000, 520000, 480000, 510000]);
 *
 * // Generate proof: "My income is at least 3x the rent"
 * const proof = await prover.proveAffordability(200000, 3); // $2000 rent
 *
 * // Share proof with landlord (contains NO actual numbers)
 * const proofJson = JSON.stringify(proof);
 *
 * // Verifier (landlord - without your private data)
 * const result = ZkProofVerifier.verify(proofJson);
 * console.log(result.valid); // true
 * console.log(result.statement); // "Income ≥ 3× monthly rent of $2000"
 * ```
 */

import init, {
  ZkFinancialProver as WasmProver,
  ZkProofVerifier as WasmVerifier,
  ZkUtils,
} from './ruvector_edge';

// ============================================================================
// Types
// ============================================================================

/**
 * A zero-knowledge proof
 */
export interface ZkProof {
  proof_type: ProofType;
  proof_data: number[];
  public_inputs: PublicInputs;
  generated_at: number;
  expires_at?: number;
}

export type ProofType =
  | 'Range'
  | 'Comparison'
  | 'Affordability'
  | 'NonNegative'
  | 'SumBound'
  | 'AverageBound'
  | 'SetMembership';

export interface PublicInputs {
  commitments: Commitment[];
  bounds: number[];
  statement: string;
  attestation?: Attestation;
}

export interface Commitment {
  point: number[];
}

export interface Attestation {
  issuer: string;
  signature: number[];
  timestamp: number;
}

export interface VerificationResult {
  valid: boolean;
  statement: string;
  verified_at: number;
  error?: string;
}

export interface RentalApplicationProof {
  income_proof: ZkProof;
  stability_proof: ZkProof;
  savings_proof?: ZkProof;
  metadata: ApplicationMetadata;
}

export interface ApplicationMetadata {
  applicant_id: string;
  property_id?: string;
  generated_at: number;
  expires_at: number;
}

// ============================================================================
// Prover (Client-Side)
// ============================================================================

/**
 * Generate zero-knowledge proofs about financial data.
 *
 * All proof generation happens locally in WebAssembly.
 * Your actual financial numbers are NEVER revealed.
 */
export class ZkFinancialProver {
  private wasmProver: WasmProver | null = null;
  private initialized = false;

  /**
   * Initialize the prover
   */
  async init(): Promise<void> {
    if (this.initialized) return;

    await init();
    this.wasmProver = new WasmProver();
    this.initialized = true;
  }

  /**
   * Load monthly income data
   * @param monthlyIncome Array of monthly income in CENTS (e.g., $6500 = 650000)
   */
  loadIncome(monthlyIncome: number[]): void {
    this.ensureInit();
    this.wasmProver!.loadIncome(new BigUint64Array(monthlyIncome.map(BigInt)));
  }

  /**
   * Load expense data for a category
   * @param category Category name (e.g., "Food", "Transportation")
   * @param monthlyExpenses Array of monthly expenses in CENTS
   */
  loadExpenses(category: string, monthlyExpenses: number[]): void {
    this.ensureInit();
    this.wasmProver!.loadExpenses(category, new BigUint64Array(monthlyExpenses.map(BigInt)));
  }

  /**
   * Load daily balance history
   * @param dailyBalances Array of daily balances in CENTS (can be negative)
   */
  loadBalances(dailyBalances: number[]): void {
    this.ensureInit();
    this.wasmProver!.loadBalances(new BigInt64Array(dailyBalances.map(BigInt)));
  }

  // --------------------------------------------------------------------------
  // Proof Generation
  // --------------------------------------------------------------------------

  /**
   * Prove: average income ≥ threshold
   *
   * Use case: Prove you make at least $X without revealing exact income
   *
   * @param thresholdDollars Minimum income threshold in dollars
   */
  async proveIncomeAbove(thresholdDollars: number): Promise<ZkProof> {
    this.ensureInit();
    const thresholdCents = Math.round(thresholdDollars * 100);
    return this.wasmProver!.proveIncomeAbove(BigInt(thresholdCents));
  }

  /**
   * Prove: income ≥ multiplier × rent
   *
   * Use case: Prove affordability for apartment application
   *
   * @param rentDollars Monthly rent in dollars
   * @param multiplier Required income multiplier (typically 3)
   */
  async proveAffordability(rentDollars: number, multiplier: number): Promise<ZkProof> {
    this.ensureInit();
    const rentCents = Math.round(rentDollars * 100);
    return this.wasmProver!.proveAffordability(BigInt(rentCents), BigInt(multiplier));
  }

  /**
   * Prove: no overdrafts in the past N days
   *
   * Use case: Prove account stability
   *
   * @param days Number of days to prove (e.g., 90)
   */
  async proveNoOverdrafts(days: number): Promise<ZkProof> {
    this.ensureInit();
    return this.wasmProver!.proveNoOverdrafts(days);
  }

  /**
   * Prove: current savings ≥ threshold
   *
   * Use case: Prove you have emergency fund
   *
   * @param thresholdDollars Minimum savings in dollars
   */
  async proveSavingsAbove(thresholdDollars: number): Promise<ZkProof> {
    this.ensureInit();
    const thresholdCents = Math.round(thresholdDollars * 100);
    return this.wasmProver!.proveSavingsAbove(BigInt(thresholdCents));
  }

  /**
   * Prove: average spending in category ≤ budget
   *
   * Use case: Prove budgeting discipline
   *
   * @param category Spending category
   * @param budgetDollars Maximum budget in dollars
   */
  async proveBudgetCompliance(category: string, budgetDollars: number): Promise<ZkProof> {
    this.ensureInit();
    const budgetCents = Math.round(budgetDollars * 100);
    return this.wasmProver!.proveBudgetCompliance(category, BigInt(budgetCents));
  }

  /**
   * Prove: debt-to-income ratio ≤ max%
   *
   * Use case: Prove creditworthiness
   *
   * @param monthlyDebtDollars Monthly debt payments in dollars
   * @param maxRatioPercent Maximum DTI ratio (e.g., 30 for 30%)
   */
  async proveDebtRatio(monthlyDebtDollars: number, maxRatioPercent: number): Promise<ZkProof> {
    this.ensureInit();
    const debtCents = Math.round(monthlyDebtDollars * 100);
    return this.wasmProver!.proveDebtRatio(BigInt(debtCents), BigInt(maxRatioPercent));
  }

  /**
   * Create complete rental application proof bundle
   *
   * Includes all proofs typically needed for rental application
   *
   * @param rentDollars Monthly rent
   * @param incomeMultiplier Required income multiple (usually 3)
   * @param stabilityDays Days of no overdrafts to prove
   * @param savingsMonths Months of rent to prove in savings (optional)
   */
  async createRentalApplication(
    rentDollars: number,
    incomeMultiplier: number = 3,
    stabilityDays: number = 90,
    savingsMonths?: number
  ): Promise<RentalApplicationProof> {
    this.ensureInit();
    const rentCents = Math.round(rentDollars * 100);
    return this.wasmProver!.createRentalApplication(
      BigInt(rentCents),
      BigInt(incomeMultiplier),
      stabilityDays,
      savingsMonths !== undefined ? BigInt(savingsMonths) : undefined
    );
  }

  private ensureInit(): void {
    if (!this.initialized || !this.wasmProver) {
      throw new Error('Prover not initialized. Call init() first.');
    }
  }
}

// ============================================================================
// Verifier (Can Run Anywhere)
// ============================================================================

/**
 * Verify zero-knowledge proofs.
 *
 * Verifier learns ONLY that the statement is true.
 * Actual numbers remain completely hidden.
 */
export class ZkProofVerifier {
  private static initialized = false;

  /**
   * Initialize the verifier
   */
  static async init(): Promise<void> {
    if (this.initialized) return;
    await init();
    this.initialized = true;
  }

  /**
   * Verify a single proof
   *
   * @param proof The proof to verify (as object or JSON string)
   */
  static async verify(proof: ZkProof | string): Promise<VerificationResult> {
    await this.init();
    const proofJson = typeof proof === 'string' ? proof : JSON.stringify(proof);
    return WasmVerifier.verify(proofJson);
  }

  /**
   * Verify a rental application bundle
   */
  static async verifyRentalApplication(
    application: RentalApplicationProof | string
  ): Promise<{ all_valid: boolean; results: VerificationResult[] }> {
    await this.init();
    const appJson = typeof application === 'string' ? application : JSON.stringify(application);
    return WasmVerifier.verifyRentalApplication(appJson);
  }

  /**
   * Get human-readable statement from proof
   */
  static async getStatement(proof: ZkProof | string): Promise<string> {
    await this.init();
    const proofJson = typeof proof === 'string' ? proof : JSON.stringify(proof);
    return WasmVerifier.getStatement(proofJson);
  }

  /**
   * Check if proof is expired
   */
  static async isExpired(proof: ZkProof | string): Promise<boolean> {
    await this.init();
    const proofJson = typeof proof === 'string' ? proof : JSON.stringify(proof);
    return WasmVerifier.isExpired(proofJson);
  }
}

// ============================================================================
// Utilities
// ============================================================================

export const ZkProofUtils = {
  /**
   * Convert proof to shareable URL
   */
  toShareableUrl(proof: ZkProof, baseUrl: string = window.location.origin): string {
    const proofJson = JSON.stringify(proof);
    return ZkUtils.proofToUrl(proofJson, baseUrl + '/verify');
  },

  /**
   * Extract proof from URL parameter
   */
  fromUrl(encoded: string): ZkProof {
    const json = ZkUtils.proofFromUrl(encoded);
    return JSON.parse(json);
  },

  /**
   * Format proof for display
   */
  formatProof(proof: ZkProof): string {
    return `
┌─────────────────────────────────────────────────┐
│ Zero-Knowledge Proof                            │
├─────────────────────────────────────────────────┤
│ Type: ${proof.proof_type.padEnd(41)}│
│ Statement: ${proof.public_inputs.statement.slice(0, 36).padEnd(36)}│
│ Generated: ${new Date(proof.generated_at * 1000).toLocaleDateString().padEnd(36)}│
│ Expires: ${proof.expires_at ? new Date(proof.expires_at * 1000).toLocaleDateString().padEnd(38) : 'Never'.padEnd(38)}│
│ Proof size: ${(proof.proof_data.length + ' bytes').padEnd(35)}│
└─────────────────────────────────────────────────┘
    `.trim();
  },

  /**
   * Calculate proof size in bytes
   */
  proofSize(proof: ZkProof): number {
    return JSON.stringify(proof).length;
  },
};

// ============================================================================
// Presets for Common Use Cases
// ============================================================================

/**
 * Pre-configured proof generators for common scenarios
 */
export const ZkPresets = {
  /**
   * Standard rental application (3x income, 90 days stability, 2 months savings)
   */
  async rentalApplication(
    prover: ZkFinancialProver,
    monthlyRent: number
  ): Promise<RentalApplicationProof> {
    return prover.createRentalApplication(monthlyRent, 3, 90, 2);
  },

  /**
   * Loan pre-qualification (income above threshold, DTI under 30%)
   */
  async loanPrequalification(
    prover: ZkFinancialProver,
    minimumIncome: number,
    monthlyDebt: number
  ): Promise<{ incomeProof: ZkProof; dtiProof: ZkProof }> {
    const incomeProof = await prover.proveIncomeAbove(minimumIncome);
    const dtiProof = await prover.proveDebtRatio(monthlyDebt, 30);
    return { incomeProof, dtiProof };
  },

  /**
   * Employment verification (income above minimum)
   */
  async employmentVerification(
    prover: ZkFinancialProver,
    minimumSalary: number
  ): Promise<ZkProof> {
    return prover.proveIncomeAbove(minimumSalary);
  },

  /**
   * Account stability (no overdrafts for 6 months)
   */
  async accountStability(prover: ZkFinancialProver): Promise<ZkProof> {
    return prover.proveNoOverdrafts(180);
  },
};

export default { ZkFinancialProver, ZkProofVerifier, ZkProofUtils, ZkPresets };
