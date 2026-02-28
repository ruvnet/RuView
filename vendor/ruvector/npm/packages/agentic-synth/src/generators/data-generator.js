/**
 * Data Generator for synthetic data creation
 */

export class DataGenerator {
  constructor(options = {}) {
    this.seed = options.seed || Date.now();
    this.format = options.format || 'json';
    this.schema = options.schema || {};
  }

  /**
   * Generate synthetic data based on schema
   * @param {number} count - Number of records to generate
   * @returns {Array} Generated data
   */
  generate(count = 1) {
    if (count < 1) {
      throw new Error('Count must be at least 1');
    }

    const data = [];
    for (let i = 0; i < count; i++) {
      data.push(this._generateRecord(i));
    }
    return data;
  }

  /**
   * Generate a single record
   * @private
   */
  _generateRecord(index) {
    const record = { id: index };

    for (const [field, config] of Object.entries(this.schema)) {
      record[field] = this._generateField(config);
    }

    return record;
  }

  /**
   * Generate a field value based on type
   * @private
   */
  _generateField(config) {
    const type = config.type || 'string';

    switch (type) {
      case 'string':
        return this._randomString(config.length || 10);
      case 'number':
        return this._randomNumber(config.min || 0, config.max || 100);
      case 'boolean':
        return Math.random() > 0.5;
      case 'array':
        return this._randomArray(config.items || 5);
      case 'vector':
        return this._randomVector(config.dimensions || 128);
      default:
        return null;
    }
  }

  _randomString(length) {
    const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    let result = '';
    for (let i = 0; i < length; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
  }

  _randomNumber(min, max) {
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }

  _randomArray(length) {
    return Array.from({ length }, (_, i) => i);
  }

  _randomVector(dimensions) {
    return Array.from({ length: dimensions }, () => Math.random());
  }

  /**
   * Set seed for reproducible generation
   */
  setSeed(seed) {
    this.seed = seed;
  }
}
