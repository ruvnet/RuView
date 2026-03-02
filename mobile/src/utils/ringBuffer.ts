export class RingBuffer<T> {
  private readonly capacity: number;
  private readonly compare?: (a: T, b: T) => number;
  private readonly values: T[] = [];

  constructor(capacity: number, compare?: (a: T, b: T) => number) {
    if (!Number.isFinite(capacity) || capacity <= 0) {
      throw new Error('RingBuffer capacity must be greater than 0');
    }
    this.capacity = Math.floor(capacity);
    this.compare = compare;
  }

  push(v: T): void {
    this.values.push(v);
    if (this.values.length > this.capacity) {
      this.values.shift();
    }
  }

  toArray(): T[] {
    return [...this.values];
  }

  clear(): void {
    this.values.length = 0;
  }

  get max(): T | null {
    if (this.values.length === 0) {
      return null;
    }
    if (!this.compare) {
      throw new Error('Comparator required for max()');
    }
    return this.values.reduce((acc, value) => (this.compare!(value, acc) > 0 ? value : acc), this.values[0]);
  }

  get min(): T | null {
    if (this.values.length === 0) {
      return null;
    }
    if (!this.compare) {
      throw new Error('Comparator required for min()');
    }
    return this.values.reduce((acc, value) => (this.compare!(value, acc) < 0 ? value : acc), this.values[0]);
  }
}
