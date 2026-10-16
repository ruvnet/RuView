// SPDX-License-Identifier: MIT

export function isPlainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function normalizedIso(value) {
  return typeof value === 'string'
    && Number.isFinite(Date.parse(value))
    && new Date(value).toISOString() === value;
}

function typeMatches(value, type) {
  if (Array.isArray(type)) return type.some((candidate) => typeMatches(value, candidate));
  if (type === 'null') return value === null;
  if (type === 'array') return Array.isArray(value);
  if (type === 'object') return isPlainObject(value);
  if (type === 'integer') return Number.isSafeInteger(value);
  if (type === 'number') return typeof value === 'number' && Number.isFinite(value);
  return typeof value === type;
}

export function validateArguments(schema, value, path = '$') {
  const errors = [];
  const type = schema.type || 'object';
  if (!typeMatches(value, type)) return [`${path} must be ${type}`];

  if (Array.isArray(type)) {
    const matched = type.find((candidate) => typeMatches(value, candidate));
    return matched ? validateArguments({ ...schema, type: matched }, value, path) : [`${path} must be ${type.join(' or ')}`];
  }

  if (type === 'object') {
    const properties = schema.properties || {};
    for (const key of schema.required || []) {
      if (!Object.hasOwn(value, key)) errors.push(`${path}.${key} is required`);
    }
    for (const [key, item] of Object.entries(value)) {
      const child = properties[key];
      if (!child) {
        if (schema.additionalProperties !== true) errors.push(`${path}.${key} is not allowed`);
      } else {
        errors.push(...validateArguments(child, item, `${path}.${key}`));
      }
    }
  }

  if (type === 'array') {
    if (schema.minItems !== undefined && value.length < schema.minItems) errors.push(`${path} has too few items`);
    if (schema.maxItems !== undefined && value.length > schema.maxItems) errors.push(`${path} has too many items`);
    if (schema.items) value.forEach((item, index) => errors.push(...validateArguments(schema.items, item, `${path}[${index}]`)));
  }

  if (schema.enum && !schema.enum.includes(value)) errors.push(`${path} must be one of ${schema.enum.join(', ')}`);
  if (type === 'string') {
    if (schema.minLength !== undefined && value.length < schema.minLength) errors.push(`${path} is too short`);
    if (schema.maxLength !== undefined && value.length > schema.maxLength) errors.push(`${path} is too long`);
    if (schema.pattern && !(new RegExp(schema.pattern, 'u')).test(value)) errors.push(`${path} has an invalid format`);
  }
  if (type === 'integer' || type === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) errors.push(`${path} is below minimum`);
    if (schema.maximum !== undefined && value > schema.maximum) errors.push(`${path} exceeds maximum`);
  }
  return errors;
}

export function assertNoCredentialFields(value, path = '$') {
  return scanSensitive(value, path).map(({ path: findingPath, rule }) => `${findingPath} contains forbidden sensitive material (${rule})`);
}
import { scanSensitive } from './sensitive.js';
