/**
 * Recursive Validation Plugin
 *
 * Validates research results recursively and triggers replanning
 * when quality thresholds are not met.
 */

import type { GoapPlugin, WorldState, PlanStep } from '../core/types';

export interface ValidationCriteria {
  minCitations?: number;
  minConfidence?: number;
  requiredDomains?: string[];
  forbiddenTerms?: string[];
  minAnswerLength?: number;
  maxContradictions?: number;
}

export class RecursiveValidationPlugin implements GoapPlugin {
  name = 'recursive-validation';
  version = '1.0.0';

  private validationCriteria: ValidationCriteria;
  private validationAttempts = 0;
  private maxValidationAttempts = 3;

  constructor(criteria: ValidationCriteria = {}) {
    this.validationCriteria = {
      minCitations: criteria.minCitations || 5,
      minConfidence: criteria.minConfidence || 0.7,
      requiredDomains: criteria.requiredDomains || [],
      forbiddenTerms: criteria.forbiddenTerms || [],
      minAnswerLength: criteria.minAnswerLength || 100,
      maxContradictions: criteria.maxContradictions || 2,
      ...criteria
    };
  }

  /**
   * Recursively validate the state
   */
  private recursiveValidate(state: WorldState, depth: number = 0): {
    valid: boolean;
    reasons: string[];
    confidence: number;
  } {
    const reasons: string[] = [];
    let confidence = 1.0;

    // Check citations count
    const citations = state.citations as any[] || [];
    if (citations.length < this.validationCriteria.minCitations!) {
      reasons.push(`Insufficient citations: ${citations.length} < ${this.validationCriteria.minCitations}`);
      confidence *= 0.5;
    }

    // Check answer length
    const answer = state.final_answer as string || '';
    if (answer.length < this.validationCriteria.minAnswerLength!) {
      reasons.push(`Answer too short: ${answer.length} < ${this.validationCriteria.minAnswerLength}`);
      confidence *= 0.6;
    }

    // Check for forbidden terms (e.g., nonsense queries)
    const forbiddenFound = this.validationCriteria.forbiddenTerms!.filter(term =>
      answer.toLowerCase().includes(term.toLowerCase())
    );
    if (forbiddenFound.length > 0) {
      reasons.push(`Forbidden terms found: ${forbiddenFound.join(', ')}`);
      confidence *= 0.3;
    }

    // Check required domains in citations
    if (this.validationCriteria.requiredDomains!.length > 0) {
      const citationDomains = citations.map(c => {
        try {
          return new URL(c.url).hostname;
        } catch {
          return '';
        }
      });

      const missingDomains = this.validationCriteria.requiredDomains!.filter(domain =>
        !citationDomains.some(cd => cd.includes(domain))
      );

      if (missingDomains.length > 0) {
        reasons.push(`Missing required domains: ${missingDomains.join(', ')}`);
        confidence *= 0.7;
      }
    }

    // Recursive validation of sub-components
    if (depth < 2) {
      // Check if we have research steps
      const researchSteps = state.research_steps as any[] || [];
      if (researchSteps.length === 0) {
        reasons.push('No research steps performed');
        confidence *= 0.4;
      } else {
        // Recursively validate each step
        researchSteps.forEach((step, i) => {
          const stepState = { ...state, ...step };
          const stepValidation = this.recursiveValidate(stepState, depth + 1);
          if (!stepValidation.valid) {
            reasons.push(`Step ${i + 1} failed validation: ${stepValidation.reasons[0]}`);
            confidence *= stepValidation.confidence;
          }
        });
      }
    }

    // Check contradictions
    const contradictions = state.contradictions as string[] || [];
    if (contradictions.length > this.validationCriteria.maxContradictions!) {
      reasons.push(`Too many contradictions: ${contradictions.length} > ${this.validationCriteria.maxContradictions}`);
      confidence *= 0.5;
    }

    // Final confidence check
    const valid = confidence >= this.validationCriteria.minConfidence!;

    if (!valid) {
      reasons.unshift(`Overall confidence ${(confidence * 100).toFixed(1)}% below threshold ${(this.validationCriteria.minConfidence! * 100)}%`);
    }

    return { valid, reasons, confidence };
  }

  hooks = {
    // Validate after synthesis
    afterSynthesize: async (result: any): Promise<void> => {
      const validation = this.recursiveValidate(result.state || result);

      if (!validation.valid) {
        this.validationAttempts++;

        console.log(`\n❌ Validation Failed (Attempt ${this.validationAttempts}/${this.maxValidationAttempts})`);
        console.log(`   Confidence: ${(validation.confidence * 100).toFixed(1)}%`);
        console.log(`   Reasons:`);
        validation.reasons.forEach(reason => {
          console.log(`   - ${reason}`);
        });

        // Force failure to trigger replanning
        if (this.validationAttempts < this.maxValidationAttempts) {
          console.log('   🔄 Triggering replan...\n');
          // Modify state to fail preconditions
          result.answer_verified = false;
          result.validation_failed = true;
          result.validation_reasons = validation.reasons;

          // This will cause the next action's preconditions to fail
          throw new Error(`Validation failed: ${validation.reasons[0]}`);
        }
      } else {
        console.log(`\n✅ Validation Passed`);
        console.log(`   Confidence: ${(validation.confidence * 100).toFixed(1)}%`);
        this.validationAttempts = 0;
      }
    },

    // Log when replanning occurs
    onReplan: async (failedStep: PlanStep, state: WorldState): Promise<void> => {
      console.log(`\n🔄 REPLANNING TRIGGERED`);
      console.log(`   Failed Step: ${failedStep.action.name}`);
      console.log(`   Validation Attempts: ${this.validationAttempts}`);

      const validationReasons = state.validation_reasons as string[] || [];
      if (validationReasons.length > 0) {
        console.log(`   Validation Issues:`);
        validationReasons.forEach(reason => {
          console.log(`   - ${reason}`);
        });
      }
    }
  };
}

// Export default instance
export default new RecursiveValidationPlugin();

// Export factory function for custom criteria
export function createValidationPlugin(criteria: ValidationCriteria): RecursiveValidationPlugin {
  return new RecursiveValidationPlugin(criteria);
}