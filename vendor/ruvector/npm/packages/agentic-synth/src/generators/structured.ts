/**
 * Structured data generator
 */

import { BaseGenerator } from './base.js';
import { GeneratorOptions, ValidationError, DataSchema, SchemaField } from '../types.js';

export class StructuredGenerator extends BaseGenerator<GeneratorOptions> {
  protected generatePrompt(options: GeneratorOptions): string {
    const { count = 10, schema, constraints, format = 'json' } = options;

    if (!schema) {
      throw new ValidationError('Schema is required for structured data generation', {
        options
      });
    }

    let prompt = `Generate ${count} realistic data records matching the following schema:

Schema:
${JSON.stringify(schema, null, 2)}

`;

    if (constraints) {
      prompt += `\nConstraints:\n${JSON.stringify(constraints, null, 2)}\n`;
    }

    prompt += `
Requirements:
1. Generate realistic, diverse data that fits the schema
2. Ensure all required fields are present
3. Follow data type constraints strictly
4. Make data internally consistent and realistic
5. Include varied but plausible values

Return ONLY a JSON array of ${count} objects, no additional text.`;

    return prompt;
  }

  protected parseResult(response: string, options: GeneratorOptions): unknown[] {
    try {
      // Extract JSON from response
      const jsonMatch = response.match(/\[[\s\S]*\]/);
      if (!jsonMatch) {
        throw new Error('No JSON array found in response');
      }

      const data = JSON.parse(jsonMatch[0]);

      if (!Array.isArray(data)) {
        throw new Error('Response is not an array');
      }

      // Validate against schema if provided
      if (options.schema) {
        return data.map((item, index) => {
          this.validateAgainstSchema(item, options.schema!, index);
          return item;
        });
      }

      return data;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';
      throw new ValidationError(`Failed to parse structured data: ${errorMessage}`, {
        response: response.substring(0, 200),
        error
      });
    }
  }

  private validateAgainstSchema(
    item: unknown,
    schema: Record<string, unknown>,
    index: number
  ): void {
    if (typeof item !== 'object' || item === null) {
      throw new ValidationError(`Item at index ${index} is not an object`, { item, schema });
    }

    const record = item as Record<string, unknown>;
    for (const [key, schemaValue] of Object.entries(schema)) {
      if (typeof schemaValue !== 'object' || schemaValue === null) continue;

      const fieldSchema = schemaValue as Record<string, unknown>;

      // Check required fields
      if (fieldSchema.required && !(key in record)) {
        throw new ValidationError(`Missing required field '${key}' at index ${index}`, {
          item,
          schema
        });
      }

      // Check types
      if (key in record && fieldSchema.type) {
        const actualType = typeof record[key];
        const expectedType = fieldSchema.type;

        if (expectedType === 'array' && !Array.isArray(record[key])) {
          throw new ValidationError(
            `Field '${key}' should be array at index ${index}`,
            { item, schema }
          );
        } else if (expectedType !== 'array' && actualType !== expectedType) {
          throw new ValidationError(
            `Field '${key}' has wrong type at index ${index}. Expected ${expectedType}, got ${actualType}`,
            { item, schema }
          );
        }
      }

      // Check nested objects
      if (fieldSchema.properties && typeof record[key] === 'object') {
        this.validateAgainstSchema(record[key], fieldSchema.properties as Record<string, unknown>, index);
      }
    }
  }

  /**
   * Generate structured data with specific domain
   */
  async generateDomain(domain: string, options: GeneratorOptions): Promise<unknown[]> {
    const domainSchemas: Record<string, DataSchema> = {
      users: {
        id: { type: 'string', required: true },
        name: { type: 'string', required: true },
        email: { type: 'string', required: true },
        age: { type: 'number', required: true },
        role: { type: 'string', required: false },
        createdAt: { type: 'string', required: true }
      },
      products: {
        id: { type: 'string', required: true },
        name: { type: 'string', required: true },
        price: { type: 'number', required: true },
        category: { type: 'string', required: true },
        inStock: { type: 'boolean', required: true },
        description: { type: 'string', required: false }
      },
      transactions: {
        id: { type: 'string', required: true },
        userId: { type: 'string', required: true },
        amount: { type: 'number', required: true },
        currency: { type: 'string', required: true },
        status: { type: 'string', required: true },
        timestamp: { type: 'string', required: true }
      }
    };

    const schema = domainSchemas[domain.toLowerCase()];
    if (!schema) {
      throw new ValidationError(`Unknown domain: ${domain}`, {
        availableDomains: Object.keys(domainSchemas)
      });
    }

    return this.generate({
      ...options,
      schema
    }).then(result => result.data);
  }

  /**
   * Generate data from JSON schema
   */
  async generateFromJSONSchema(jsonSchema: Record<string, unknown>, options: GeneratorOptions): Promise<unknown[]> {
    // Convert JSON Schema to internal schema format
    const schema = this.convertJSONSchema(jsonSchema);

    return this.generate({
      ...options,
      schema
    }).then(result => result.data);
  }

  private convertJSONSchema(jsonSchema: Record<string, unknown>): DataSchema {
    const schema: DataSchema = {};

    if (jsonSchema.properties && typeof jsonSchema.properties === 'object') {
      const properties = jsonSchema.properties as Record<string, unknown>;
      for (const [key, value] of Object.entries(properties)) {
        if (typeof value !== 'object' || value === null) continue;

        const prop = value as Record<string, unknown>;
        const field: SchemaField = {
          type: typeof prop.type === 'string' ? prop.type : 'string',
          required: Array.isArray(jsonSchema.required) && jsonSchema.required.includes(key) || false
        };

        if (prop.properties) {
          field.properties = this.convertJSONSchema(prop);
        }

        schema[key] = field;
      }
    }

    return schema;
  }
}
