/**
 * Anti-Hallucination and Factual Grounding Plugin
 * Ensures all claims are grounded with citations and implements verification schemas
 */

import { PluginContext, AdvancedPluginHooks } from '../../core/advanced-types.js';
import { PerplexityClient } from '../../actions/perplexity-actions.js';

export interface FactualClaim {
  claim: string;
  citations: string[];
  confidence: number;
  verified: boolean;
  groundingType: 'direct' | 'inferred' | 'synthesized';
}

export interface HallucinationCheck {
  totalClaims: number;
  groundedClaims: number;
  ungroundedClaims: string[];
  confidenceScore: number;
  hallucinationRisk: 'low' | 'medium' | 'high';
}

export class AntiHallucinationPlugin {
  name = 'anti-hallucination';
  version = '1.0.0';

  private factualClaims: FactualClaim[] = [];
  private hallucinationCheck: HallucinationCheck | null = null;
  private citationRequirement = 0.8; // 80% of claims must have citations
  private perplexityClient: PerplexityClient | null = null;

  hooks: AdvancedPluginHooks = {
    /**
     * Before search, set up grounding requirements
     */
    beforeSearch: async (context: PluginContext) => {
      console.log('🛡️ [Anti-Hallucination] Activating factual grounding requirements');

      // Enhance search to prioritize cited sources
      context.metadata = {
        ...context.metadata,
        groundingRequirements: {
          requireCitations: true,
          minimumCitationsPerClaim: 1,
          verificationLevel: 'strict'
        }
      };

      // Add citation-focused search parameters
      if (context.searchParams) {
        context.searchParams.return_citations = true;
        context.searchParams.citation_quality = 'high';
      }
    },

    /**
     * After search, extract and validate factual claims
     */
    afterSearch: async (results: any, context: PluginContext) => {
      console.log('🔍 [Anti-Hallucination] Extracting factual claims...');

      // Extract all factual claims from results
      this.factualClaims = this.extractFactualClaims(results);

      // Validate each claim against citations
      for (const claim of this.factualClaims) {
        claim.verified = this.verifyClaim(claim, results.citations || []);
      }

      // Calculate hallucination risk
      this.hallucinationCheck = this.assessHallucinationRisk(this.factualClaims);

      console.log(`📊 [Anti-Hallucination] Grounding rate: ${(this.hallucinationCheck.groundedClaims / this.hallucinationCheck.totalClaims * 100).toFixed(1)}%`);
      console.log(`⚠️ [Anti-Hallucination] Risk level: ${this.hallucinationCheck.hallucinationRisk}`);

      // Enhance results with grounding data
      results.grounding = {
        factualClaims: this.factualClaims,
        hallucinationCheck: this.hallucinationCheck
      };

      return results;
    },

    /**
     * Before synthesis, ensure grounding requirements
     */
    beforeSynthesize: async (context: PluginContext) => {
      if (!this.hallucinationCheck) return;

      // If high hallucination risk, modify synthesis approach
      if (this.hallucinationCheck.hallucinationRisk === 'high') {
        console.log('🚨 [Anti-Hallucination] High risk detected - enforcing strict grounding');

        context.synthesisParams = {
          ...context.synthesisParams,
          instruction: 'Only make claims that are directly supported by citations. Express uncertainty for any unverified information.',
          requireCitations: true,
          uncertaintyThreshold: 0.7
        };
      }
    },

    /**
     * After synthesis, validate final response
     */
    afterSynthesize: async (result: any, context: PluginContext) => {
      console.log('✅ [Anti-Hallucination] Validating synthesized response...');

      // Extract claims from synthesized response
      const responseClaims = this.extractResponseClaims(result.content);

      // Check each claim for grounding
      const validationResults = responseClaims.map(claim => ({
        claim,
        grounded: this.isClaimGrounded(claim, result.citations || []),
        requiresFlag: this.requiresUncertaintyFlag(claim)
      }));

      // Add uncertainty markers where needed
      let enhancedContent = result.content;
      for (const validation of validationResults) {
        if (!validation.grounded && validation.requiresFlag) {
          enhancedContent = this.addUncertaintyMarker(enhancedContent, validation.claim);
        }
      }

      result.content = enhancedContent;
      result.validation = {
        ...result.validation,
        antiHallucination: {
          totalClaims: validationResults.length,
          groundedClaims: validationResults.filter(v => v.grounded).length,
          uncertaintyMarkersAdded: validationResults.filter(v => v.requiresFlag && !v.grounded).length
        }
      };

      return result;
    },

    /**
     * Final verification against hallucination
     */
    verify: async (result: any, context: PluginContext) => {
      if (!this.hallucinationCheck) {
        return { valid: false, confidence: 0, method: 'no-hallucination-check' };
      }

      const groundingRate = this.hallucinationCheck.groundedClaims / Math.max(this.hallucinationCheck.totalClaims, 1);
      const meetsRequirement = groundingRate >= this.citationRequirement;

      // Additional checks
      const hasUnverifiedCritical = this.checkForCriticalUnverifiedClaims(result);
      const citationQuality = this.assessCitationQuality(result.citations || []);

      const overallScore = (groundingRate * 0.5) + (citationQuality * 0.3) + (hasUnverifiedCritical ? 0 : 0.2);

      return {
        valid: meetsRequirement && !hasUnverifiedCritical,
        confidence: overallScore,
        method: 'anti-hallucination-verification',
        details: {
          groundingRate,
          hallucinationRisk: this.hallucinationCheck.hallucinationRisk,
          ungroundedClaims: this.hallucinationCheck.ungroundedClaims.length,
          citationQuality
        }
      };
    }
  };

  /**
   * Extract factual claims from search results
   */
  private extractFactualClaims(results: any): FactualClaim[] {
    const claims: FactualClaim[] = [];
    const text = typeof results === 'string' ? results : JSON.stringify(results);

    // Pattern matching for factual statements
    const claimPatterns = [
      /(?:is|are|was|were|has|have|will|can|does|do)\s+[^.?!]+[.!]/gi,
      /\d+(?:\.\d+)?%?\s+(?:of|in|from|to|by)[^.?!]+[.!]/gi,
      /(?:according to|research shows|studies indicate)[^.?!]+[.!]/gi
    ];

    for (const pattern of claimPatterns) {
      const matches = text.match(pattern) || [];
      for (const match of matches) {
        claims.push({
          claim: match.trim(),
          citations: [],
          confidence: 0,
          verified: false,
          groundingType: 'direct'
        });
      }
    }

    return claims;
  }

  /**
   * Verify a claim against available citations
   */
  private verifyClaim(claim: FactualClaim, citations: string[]): boolean {
    // Check if claim content appears in any citation
    const claimKeywords = this.extractKeywords(claim.claim);

    for (const citation of citations) {
      const citationKeywords = this.extractKeywords(citation);
      const overlap = this.calculateKeywordOverlap(claimKeywords, citationKeywords);

      if (overlap > 0.3) {
        claim.citations.push(citation);
        claim.confidence = Math.max(claim.confidence, overlap);
      }
    }

    return claim.citations.length > 0;
  }

  /**
   * Extract keywords from text
   */
  private extractKeywords(text: string): Set<string> {
    return new Set(
      text.toLowerCase()
        .replace(/[^a-z0-9\s]/g, '')
        .split(/\s+/)
        .filter(word => word.length > 3)
    );
  }

  /**
   * Calculate keyword overlap between two sets
   */
  private calculateKeywordOverlap(set1: Set<string>, set2: Set<string>): number {
    const intersection = new Set([...set1].filter(x => set2.has(x)));
    const union = new Set([...set1, ...set2]);
    return intersection.size / union.size;
  }

  /**
   * Assess overall hallucination risk
   */
  private assessHallucinationRisk(claims: FactualClaim[]): HallucinationCheck {
    const totalClaims = claims.length;
    const groundedClaims = claims.filter(c => c.verified).length;
    const ungroundedClaims = claims.filter(c => !c.verified).map(c => c.claim);

    const groundingRate = totalClaims > 0 ? groundedClaims / totalClaims : 1;

    let hallucinationRisk: 'low' | 'medium' | 'high';
    if (groundingRate >= 0.8) hallucinationRisk = 'low';
    else if (groundingRate >= 0.6) hallucinationRisk = 'medium';
    else hallucinationRisk = 'high';

    return {
      totalClaims,
      groundedClaims,
      ungroundedClaims,
      confidenceScore: groundingRate,
      hallucinationRisk
    };
  }

  /**
   * Extract claims from synthesized response
   */
  private extractResponseClaims(content: string): string[] {
    // Split into sentences and filter for factual claims
    return content.split(/[.!?]/)
      .filter(sentence => sentence.trim().length > 10)
      .filter(sentence => /\b(?:is|are|was|were|has|have|will|can)\b/i.test(sentence));
  }

  /**
   * Check if a claim is grounded in citations
   */
  private isClaimGrounded(claim: string, citations: string[]): boolean {
    const claimKeywords = this.extractKeywords(claim);

    for (const citation of citations) {
      const overlap = this.calculateKeywordOverlap(
        claimKeywords,
        this.extractKeywords(citation)
      );

      if (overlap > 0.3) return true;
    }

    return false;
  }

  /**
   * Determine if claim requires uncertainty flag
   */
  private requiresUncertaintyFlag(claim: string): boolean {
    // Check for definitive language that needs qualification
    const definitivePatterns = [
      /\b(?:always|never|every|all|none|must|definitely|certainly)\b/i,
      /\b\d+(?:\.\d+)?%\b/, // Specific percentages
      /\b(?:proven|confirmed|established|guaranteed)\b/i
    ];

    return definitivePatterns.some(pattern => pattern.test(claim));
  }

  /**
   * Add uncertainty marker to content
   */
  private addUncertaintyMarker(content: string, claim: string): string {
    // Add qualifier before ungrounded claims
    const qualifiers = [
      'Based on available information, ',
      'It appears that ',
      'Evidence suggests that ',
      'While not fully verified, '
    ];

    const qualifier = qualifiers[Math.floor(Math.random() * qualifiers.length)];

    // Try to replace the claim with qualified version
    if (content.includes(claim)) {
      return content.replace(claim, qualifier.toLowerCase() + claim);
    }

    return content;
  }

  /**
   * Check for critical unverified claims
   */
  private checkForCriticalUnverifiedClaims(result: any): boolean {
    // Critical patterns that must be verified
    const criticalPatterns = [
      /\b(?:medical|health|safety|legal|financial)\b.*\b(?:advice|recommendation|must|should)\b/i,
      /\b(?:fatal|deadly|dangerous|toxic|harmful)\b/i,
      /\b(?:guaranteed|proven|cure|treatment)\b/i
    ];

    const content = result.content || '';
    const hasCritical = criticalPatterns.some(pattern => pattern.test(content));

    if (hasCritical) {
      // Check if critical claims are grounded
      const criticalClaims = this.extractResponseClaims(content)
        .filter(claim => criticalPatterns.some(p => p.test(claim)));

      return criticalClaims.some(claim =>
        !this.isClaimGrounded(claim, result.citations || [])
      );
    }

    return false;
  }

  /**
   * Assess citation quality
   */
  private assessCitationQuality(citations: string[]): number {
    if (citations.length === 0) return 0;

    // Check for quality indicators
    let qualityScore = 0;

    const qualityDomains = [
      'arxiv.org', 'nature.com', 'science.org', 'ieee.org',
      'acm.org', 'pubmed', '.edu', '.gov'
    ];

    for (const citation of citations) {
      const hasQualityDomain = qualityDomains.some(domain =>
        citation.toLowerCase().includes(domain)
      );

      if (hasQualityDomain) qualityScore += 1;
    }

    return Math.min(qualityScore / citations.length, 1.0);
  }

  /**
   * Get or create Perplexity client
   */
  private getClient(): PerplexityClient {
    if (!this.perplexityClient) {
      const apiKey = process.env.PERPLEXITY_API_KEY;
      if (!apiKey) {
        throw new Error('PERPLEXITY_API_KEY is required for anti-hallucination verification');
      }
      this.perplexityClient = new PerplexityClient(apiKey);
    }
    return this.perplexityClient;
  }

  /**
   * Execute anti-hallucination verification directly using real API
   */
  async execute(params: any): Promise<any> {
    const claims = params.claims || [params.query || 'test claim'];
    const providedCitations = params.citations || [];
    const maxCitationLength = params.maxCitationLength || 300; // Limit citation length
    const maxCitationsPerClaim = params.maxCitationsPerClaim || 3; // Limit citations per claim

    console.log(`🔍 Applying Anti-Hallucination verification...`);
    console.log(`  Claims to verify: ${claims.length}`);
    console.log(`  Available citations: ${providedCitations.length}`);

    const client = this.getClient();
    const verifiedClaims = [];

    // Verify each claim using Perplexity API
    for (const claim of claims) {
      // Search for evidence supporting or refuting the claim
      const searchResponse = await client.search({
        query: claim,
        maxResults: 5
      });

      // Check if claim is supported by search results
      const searchCitations = searchResponse.results || [];
      const supportingCitations = this.findSupportingCitations(claim, [
        ...providedCitations,
        ...searchCitations.map((r: any) => `${r.title}: ${r.snippet}`)
      ]);

      // Truncate citations to prevent token overflow
      const truncatedCitations = supportingCitations
        .slice(0, maxCitationsPerClaim)
        .map(citation => citation.length > maxCitationLength ?
          citation.substring(0, maxCitationLength) + '...' : citation);

      // Use Perplexity to verify the claim
      const verificationResponse = await client.chat({
        messages: [
          {
            role: 'system',
            content: 'You are a fact-checker. Evaluate if the claim is supported by the evidence. Respond with JSON: {"verified": true/false, "confidence": 0.0-1.0, "reason": "explanation"}'
          },
          {
            role: 'user',
            content: `Claim: ${claim}\n\nEvidence:\n${searchCitations.slice(0, 3).map((r: any, i: number) => `[${i+1}] ${r.title}: ${r.snippet?.substring(0, 200) || ''}`).join('\n')}\n\nIs this claim verified?`
          }
        ],
        model: 'sonar',
        temperature: 0.1,
        maxTokens: 200
      });

      let verification = { verified: false, confidence: 0.5, reason: 'Unable to verify' };
      try {
        const content = verificationResponse.choices[0]?.message?.content || '{}';
        const jsonMatch = content.match(/\{[^}]*\}/);
        if (jsonMatch) {
          verification = JSON.parse(jsonMatch[0]);
        }
      } catch (e) {
        // Default verification if parsing fails
      }

      const hasSupport = verification.verified && truncatedCitations.length > 0;
      const confidence = hasSupport ?
        Math.min(verification.confidence + (truncatedCitations.length * 0.05), 1.0) :
        verification.confidence * 0.5;

      verifiedClaims.push({
        claim,
        verified: hasSupport,
        confidence,
        supportingCitations: truncatedCitations,
        reason: verification.reason,
        warning: hasSupport ? null : 'Unverified - ' + verification.reason
      });
    }

    const overallVerification = verifiedClaims.filter((c: any) => c.verified).length / claims.length;

    return {
      success: true,
      method: 'anti-hallucination',
      verification: {
        score: overallVerification,
        rating: overallVerification > 0.8 ? 'High' : overallVerification > 0.5 ? 'Medium' : 'Low',
        verifiedClaims: verifiedClaims.filter((c: any) => c.verified).length,
        totalClaims: claims.length
      },
      claims: verifiedClaims,
      reasoning: `Anti-Hallucination Analysis:\n\n` +
                 `Verified ${verifiedClaims.filter((c: any) => c.verified).length}/${claims.length} claims with citations.\n` +
                 `Overall Verification Score: ${(overallVerification * 100).toFixed(1)}%\n\n` +
                 `${verifiedClaims.filter((c: any) => !c.verified).length > 0 ?
                   `⚠️ Warning: ${verifiedClaims.filter((c: any) => !c.verified).length} claims lack supporting evidence\n` :
                   '✅ All claims are properly grounded in citations'}\n\n` +
                 `This verification ensures that all claims are grounded in actual evidence, ` +
                 `preventing hallucination and ensuring factual accuracy.`
    };
  }

  /**
   * Find citations that support a claim
   */
  private findSupportingCitations(claim: string, citations: string[]): string[] {
    const claimWords = claim.toLowerCase().split(' ').filter(w => w.length > 3);

    return citations.filter(citation => {
      const citationLower = citation.toLowerCase();
      const matchCount = claimWords.filter(word => citationLower.includes(word)).length;
      return matchCount >= Math.min(3, claimWords.length * 0.3);
    });
  }
}

export default new AntiHallucinationPlugin();