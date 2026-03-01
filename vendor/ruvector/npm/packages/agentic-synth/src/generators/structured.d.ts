/**
 * Structured data generator
 */
import { BaseGenerator } from './base.js';
import { GeneratorOptions } from '../types.js';
export declare class StructuredGenerator extends BaseGenerator<GeneratorOptions> {
    protected generatePrompt(options: GeneratorOptions): string;
    protected parseResult(response: string, options: GeneratorOptions): unknown[];
    private validateAgainstSchema;
    /**
     * Generate structured data with specific domain
     */
    generateDomain(domain: string, options: GeneratorOptions): Promise<unknown[]>;
    /**
     * Generate data from JSON schema
     */
    generateFromJSONSchema(jsonSchema: Record<string, unknown>, options: GeneratorOptions): Promise<unknown[]>;
    private convertJSONSchema;
}
//# sourceMappingURL=structured.d.ts.map