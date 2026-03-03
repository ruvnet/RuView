/**
 * Critical Reasoning Validator Plugin
 *
 * Applies critical reasoning to validate content accuracy and logical consistency.
 * Forces replanning when reasoning detects issues.
 */

import type { GoapPlugin, WorldState, PlanStep } from '../core/types';

interface ReasoningCheck {
  type: 'logical' | 'factual' | 'coherence' | 'relevance' | 'completeness';
  description: string;
  validator: (content: string, citations: any[]) => Promise<{ valid: boolean; issues: string[] }>;
}

export class CriticalReasoningValidator implements GoapPlugin {
  name = 'critical-reasoning-validator';
  version = '1.0.0';

  private replanningTriggered = false;
  private validationDepth = 0;
  private maxDepth = 3;

  /**
   * Critical reasoning checks
   */
  private reasoningChecks: ReasoningCheck[] = [
    {
      type: 'logical',
      description: 'Check for logical contradictions and fallacies',
      validator: async (content: string, citations: any[]) => {
        const issues: string[] = [];

        // Check for self-contradictions
        const sentences = content.split(/[.!?]+/).filter(s => s.trim().length > 10);
        const contradictionPatterns = [
          { pattern: /both (.+) and not \1/i, issue: 'Direct contradiction detected' },
          { pattern: /always (.+) but sometimes not/i, issue: 'Temporal contradiction' },
          { pattern: /definitely (.+) but possibly not/i, issue: 'Certainty contradiction' },
          { pattern: /proven (.+) but no evidence/i, issue: 'Evidence contradiction' }
        ];

        for (const { pattern, issue } of contradictionPatterns) {
          if (pattern.test(content)) {
            issues.push(issue);
          }
        }

        // Check for circular reasoning
        const firstSentence = sentences[0]?.toLowerCase() || '';
        const lastSentence = sentences[sentences.length - 1]?.toLowerCase() || '';
        if (firstSentence && lastSentence &&
            this.calculateSimilarity(firstSentence, lastSentence) > 0.8) {
          issues.push('Potential circular reasoning detected');
        }

        // Check for non-sequiturs (conclusions that don't follow)
        const conclusionMarkers = ['therefore', 'thus', 'hence', 'so', 'consequently'];
        for (const marker of conclusionMarkers) {
          const conclusionIndex = content.toLowerCase().indexOf(marker);
          if (conclusionIndex > 0) {
            const beforeConclusion = content.substring(Math.max(0, conclusionIndex - 200), conclusionIndex);
            const afterConclusion = content.substring(conclusionIndex, conclusionIndex + 200);

            // Check if conclusion relates to premises
            if (this.calculateSimilarity(beforeConclusion, afterConclusion) < 0.3) {
              issues.push(`Conclusion after "${marker}" may not follow from premises`);
            }
          }
        }

        return { valid: issues.length === 0, issues };
      }
    },
    {
      type: 'factual',
      description: 'Verify factual claims against citations',
      validator: async (content: string, citations: any[]) => {
        const issues: string[] = [];

        // Extract claims that should be verifiable
        const claimPatterns = [
          /(\d+(?:\.\d+)?%)/g,  // Percentages
          /\$[\d,]+(?:\.\d+)?(?:\s*(?:billion|million|thousand))?/gi,  // Money amounts
          /\b\d{4}\b/g,  // Years
          /\b(?:increased?|decreased?|grew|fell|rose|dropped)\s+(?:by\s+)?(\d+(?:\.\d+)?%?)/gi,  // Changes
          /(?:first|last|only|largest|smallest|most|least)\s+\w+/gi,  // Superlatives
        ];

        let unverifiedClaims = 0;
        let totalClaims = 0;
        for (const pattern of claimPatterns) {
          const matches = content.match(pattern) || [];
          totalClaims += matches.length;
          for (const claim of matches) {
            // Check if claim appears in any citation snippet
            const verified = citations.some(c =>
              c.snippet && c.snippet.includes(claim.replace(/\$/g, ''))
            );
            if (!verified) {
              unverifiedClaims++;
            }
          }
        }

        const verificationRate = totalClaims > 0
          ? (totalClaims - unverifiedClaims) / totalClaims
          : 1;

        if (verificationRate < 0.6) {
          issues.push(`Low fact verification rate: ${(verificationRate * 100).toFixed(1)}%`);
        }

        // Check for impossible claims
        const impossiblePatterns = [
          { pattern: /more than 100%/i, issue: 'Impossible percentage claim' },
          { pattern: /negative probability/i, issue: 'Impossible probability' },
          { pattern: /before the big bang/i, issue: 'Impossible temporal claim' },
          { pattern: /faster than light communication/i, issue: 'Physically impossible claim' }
        ];

        for (const { pattern, issue } of impossiblePatterns) {
          if (pattern.test(content)) {
            issues.push(issue);
          }
        }

        return { valid: issues.length === 0, issues };
      }
    },
    {
      type: 'coherence',
      description: 'Check content coherence and consistency',
      validator: async (content: string, citations: any[]) => {
        const issues: string[] = [];

        // Check topic coherence
        const paragraphs = content.split(/\n\n+/).filter(p => p.length > 50);
        if (paragraphs.length > 1) {
          let topicShifts = 0;
          for (let i = 1; i < paragraphs.length; i++) {
            const similarity = this.calculateSimilarity(paragraphs[i-1], paragraphs[i]);
            if (similarity < 0.2) {
              topicShifts++;
            }
          }

          if (topicShifts > paragraphs.length / 2) {
            issues.push('Content lacks coherence - too many topic shifts');
          }
        }

        // Check for incomplete thoughts
        const incompletePatterns = [
          /\b(?:such as|including|for example|e\.g\.|i\.e\.)\s*$/i,
          /\b(?:because|since|although|however|therefore)\s*$/i,
          /\b(?:first|second|third|finally)\s*$/i
        ];

        for (const pattern of incompletePatterns) {
          if (pattern.test(content.trim())) {
            issues.push('Content appears to end with incomplete thought');
          }
        }

        return { valid: issues.length === 0, issues };
      }
    },
    {
      type: 'relevance',
      description: 'Check if content addresses the query',
      validator: async (content: string, citations: any[]) => {
        const issues: string[] = [];

        // Check if the response is generic/boilerplate
        const genericPhrases = [
          'i cannot provide information',
          'no information available',
          'unable to find',
          'does not exist',
          'made-up',
          'fictional',
          'not real'
        ];

        const genericCount = genericPhrases.filter(phrase =>
          content.toLowerCase().includes(phrase)
        ).length;

        if (genericCount > 2) {
          issues.push('Response appears to be generic/avoidant rather than researched');
        }

        // Check citation relevance
        if (citations.length > 0) {
          const irrelevantCitations = citations.filter(c => {
            // Check if citation title/snippet relates to content
            const relevance = this.calculateSimilarity(
              content.substring(0, 500),
              (c.title || '') + ' ' + (c.snippet || '')
            );
            return relevance < 0.1;
          });

          if (irrelevantCitations.length > citations.length / 2) {
            issues.push('Many citations appear irrelevant to the content');
          }
        }

        return { valid: issues.length === 0, issues };
      }
    },
    {
      type: 'completeness',
      description: 'Check if critical aspects are addressed',
      validator: async (content: string, citations: any[]) => {
        const issues: string[] = [];

        // Check for balanced perspective
        const perspectiveMarkers = {
          positive: ['advantage', 'benefit', 'positive', 'good', 'success', 'pro'],
          negative: ['disadvantage', 'risk', 'negative', 'bad', 'failure', 'con'],
          neutral: ['however', 'although', 'but', 'on the other hand', 'alternatively']
        };

        const posCount = perspectiveMarkers.positive.filter(m =>
          content.toLowerCase().includes(m)).length;
        const negCount = perspectiveMarkers.negative.filter(m =>
          content.toLowerCase().includes(m)).length;
        const neutralCount = perspectiveMarkers.neutral.filter(m =>
          content.toLowerCase().includes(m)).length;

        if ((posCount > 5 && negCount === 0) || (negCount > 5 && posCount === 0)) {
          issues.push('Content appears one-sided, lacking balanced perspective');
        }

        if (neutralCount === 0 && content.length > 1000) {
          issues.push('Long content lacks nuance or alternative viewpoints');
        }

        // Check for missing critical components
        const questionWords = ['who', 'what', 'when', 'where', 'why', 'how'];
        const addressedQuestions = questionWords.filter(q =>
          content.toLowerCase().includes(q));

        if (addressedQuestions.length < 2 && content.length > 500) {
          issues.push('Content may be missing critical aspects (who/what/when/where/why/how)');
        }

        return { valid: issues.length === 0, issues };
      }
    }
  ];

  /**
   * Calculate similarity between two strings (0-1)
   */
  private calculateSimilarity(str1: string, str2: string): number {
    const words1 = new Set(str1.toLowerCase().split(/\s+/));
    const words2 = new Set(str2.toLowerCase().split(/\s+/));

    const intersection = new Set([...words1].filter(x => words2.has(x)));
    const union = new Set([...words1, ...words2]);

    return union.size > 0 ? intersection.size / union.size : 0;
  }

  /**
   * Perform recursive critical reasoning validation
   */
  private async performCriticalValidation(
    state: WorldState,
    depth: number = 0
  ): Promise<{
    valid: boolean;
    confidence: number;
    criticalIssues: string[];
  }> {
    const content = state.final_answer as string || '';
    const citations = state.citations as any[] || [];
    const allIssues: string[] = [];
    let failedChecks = 0;

    console.log(`\n🧠 Critical Reasoning Validation (Depth ${depth + 1}/${this.maxDepth})`);

    for (const check of this.reasoningChecks) {
      const result = await check.validator(content, citations);

      if (!result.valid) {
        failedChecks++;
        console.log(`   ❌ ${check.type.toUpperCase()}: Failed`);
        result.issues.forEach(issue => {
          console.log(`      - ${issue}`);
          allIssues.push(`[${check.type}] ${issue}`);
        });
      } else {
        console.log(`   ✅ ${check.type.toUpperCase()}: Passed`);
      }
    }

    // Calculate confidence based on passed checks
    const confidence = (this.reasoningChecks.length - failedChecks) / this.reasoningChecks.length;

    // Recursive validation if we have sub-components and haven't reached max depth
    if (depth < this.maxDepth - 1) {
      const researchSteps = state.research_steps as any[] || [];
      if (researchSteps.length > 0) {
        console.log(`\n   📊 Validating ${researchSteps.length} research steps...`);
        for (let i = 0; i < Math.min(researchSteps.length, 3); i++) {
          const stepValidation = await this.performCriticalValidation(
            { ...state, final_answer: researchSteps[i].content || '' },
            depth + 1
          );

          if (!stepValidation.valid) {
            allIssues.push(`Step ${i + 1}: ${stepValidation.criticalIssues[0]}`);
          }
        }
      }
    }

    const valid = confidence >= 0.6; // 60% of checks must pass

    return {
      valid,
      confidence,
      criticalIssues: allIssues
    };
  }

  hooks = {
    // Validate after synthesis with critical reasoning
    afterSynthesize: async (result: any): Promise<void> => {
      this.validationDepth++;

      const validation = await this.performCriticalValidation(
        result.state || result
      );

      console.log(`\n📊 Critical Reasoning Summary:`);
      console.log(`   Confidence: ${(validation.confidence * 100).toFixed(1)}%`);
      console.log(`   Valid: ${validation.valid ? '✅ Yes' : '❌ No'}`);

      if (!validation.valid) {
        console.log(`   Issues Found: ${validation.criticalIssues.length}`);

        if (this.validationDepth < this.maxDepth && !this.replanningTriggered) {
          console.log(`\n🔄 FORCING REPLAN due to critical reasoning failures`);
          this.replanningTriggered = true;

          // Modify state to force replanning
          result.validation_failed = true;
          result.critical_issues = validation.criticalIssues;
          result.answer_verified = false;

          // This will cause the next action to fail
          throw new Error(`Critical reasoning failed: ${validation.criticalIssues[0]}`);
        }
      } else {
        console.log(`   ✨ Content passes critical reasoning checks`);
        this.replanningTriggered = false;
        this.validationDepth = 0;
      }
    },

    onReplan: async (failedStep: PlanStep, state: WorldState): Promise<void> => {
      console.log(`\n🔄 CRITICAL REASONING TRIGGERED REPLAN`);
      console.log(`   Failed at: ${failedStep.action.name}`);

      const issues = state.critical_issues as string[] || [];
      if (issues.length > 0) {
        console.log(`   Critical Issues to Address:`);
        issues.slice(0, 5).forEach(issue => {
          console.log(`   🔍 ${issue}`);
        });
      }

      console.log(`   Strategy: Adjusting search parameters for better results`);
      this.validationDepth = 0; // Reset for new plan
    }
  };
}

// Export default instance
export default new CriticalReasoningValidator();

// Export factory function
export function createCriticalValidator(): CriticalReasoningValidator {
  return new CriticalReasoningValidator();
}