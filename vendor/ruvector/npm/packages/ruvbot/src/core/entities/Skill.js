"use strict";
/**
 * Skill entity - represents an executable skill/capability
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SkillEntity = void 0;
const zod_1 = require("zod");
class SkillEntity {
    constructor(options) {
        this.definition = options.definition;
        this.executor = options.executor;
        this.inputSchema = options.inputSchema;
        this.outputSchema = options.outputSchema;
    }
    /**
     * Get skill name
     */
    get name() {
        return this.definition.name;
    }
    /**
     * Get skill description
     */
    get description() {
        return this.definition.description;
    }
    /**
     * Get skill version
     */
    get version() {
        return this.definition.version;
    }
    /**
     * Get skill inputs
     */
    get inputs() {
        return this.definition.inputs;
    }
    /**
     * Get skill outputs
     */
    get outputs() {
        return this.definition.outputs;
    }
    /**
     * Get skill examples
     */
    get examples() {
        return this.definition.examples ?? [];
    }
    /**
     * Validate input against schema
     */
    validateInput(input) {
        if (!this.inputSchema) {
            return { valid: true };
        }
        try {
            this.inputSchema.parse(input);
            return { valid: true };
        }
        catch (error) {
            if (error instanceof zod_1.z.ZodError) {
                return {
                    valid: false,
                    errors: error.errors.map((e) => `${e.path.join('.')}: ${e.message}`),
                };
            }
            throw error;
        }
    }
    /**
     * Validate output against schema
     */
    validateOutput(output) {
        if (!this.outputSchema) {
            return { valid: true };
        }
        try {
            this.outputSchema.parse(output);
            return { valid: true };
        }
        catch (error) {
            if (error instanceof zod_1.z.ZodError) {
                return {
                    valid: false,
                    errors: error.errors.map((e) => `${e.path.join('.')}: ${e.message}`),
                };
            }
            throw error;
        }
    }
    /**
     * Execute the skill
     */
    async execute(input, context) {
        const startTime = Date.now();
        // Validate input
        const inputValidation = this.validateInput(input);
        if (!inputValidation.valid) {
            return {
                success: false,
                error: `Input validation failed: ${inputValidation.errors?.join(', ')}`,
                metadata: { latency: Date.now() - startTime },
            };
        }
        try {
            // Execute skill
            const result = await this.executor(input, context);
            // Validate output if successful
            if (result.success && result.data) {
                const outputValidation = this.validateOutput(result.data);
                if (!outputValidation.valid) {
                    return {
                        success: false,
                        error: `Output validation failed: ${outputValidation.errors?.join(', ')}`,
                        metadata: { latency: Date.now() - startTime },
                    };
                }
            }
            return {
                ...result,
                metadata: {
                    ...result.metadata,
                    latency: Date.now() - startTime,
                },
            };
        }
        catch (error) {
            const errorMessage = error instanceof Error ? error.message : String(error);
            return {
                success: false,
                error: errorMessage,
                metadata: { latency: Date.now() - startTime },
            };
        }
    }
    /**
     * Generate tool definition for LLM
     */
    toLLMTool() {
        const properties = {};
        const required = [];
        for (const input of this.definition.inputs) {
            properties[input.name] = {
                type: input.type,
                description: input.description,
            };
            if (input.required) {
                required.push(input.name);
            }
        }
        return {
            name: this.definition.name,
            description: this.definition.description,
            parameters: {
                type: 'object',
                properties,
                required,
            },
        };
    }
    /**
     * Serialize skill definition to JSON
     */
    toJSON() {
        return this.definition;
    }
    /**
     * Create a simple skill from a function
     */
    static create(name, description, executor, options) {
        return new SkillEntity({
            definition: {
                name,
                description,
                version: '1.0.0',
                inputs: options?.inputs ?? [],
                outputs: options?.outputs ?? [],
            },
            executor,
            inputSchema: options?.inputSchema,
            outputSchema: options?.outputSchema,
        });
    }
}
exports.SkillEntity = SkillEntity;
//# sourceMappingURL=Skill.js.map