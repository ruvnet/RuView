import { RingBuffer } from '@/utils/ringBuffer';

describe('RingBuffer', () => {
  describe('constructor', () => {
    it('creates a buffer with the given capacity', () => {
      const buf = new RingBuffer<number>(5);
      expect(buf.toArray()).toEqual([]);
    });

    it('floors fractional capacity', () => {
      const buf = new RingBuffer<number>(3.9);
      buf.push(1);
      buf.push(2);
      buf.push(3);
      buf.push(4);
      // capacity is 3 (floored), so oldest is evicted
      expect(buf.toArray()).toEqual([2, 3, 4]);
    });

    it('throws on zero capacity', () => {
      expect(() => new RingBuffer<number>(0)).toThrow('capacity must be greater than 0');
    });

    it('throws on negative capacity', () => {
      expect(() => new RingBuffer<number>(-1)).toThrow('capacity must be greater than 0');
    });

    it('throws on NaN capacity', () => {
      expect(() => new RingBuffer<number>(NaN)).toThrow('capacity must be greater than 0');
    });

    it('throws on Infinity capacity', () => {
      expect(() => new RingBuffer<number>(Infinity)).toThrow('capacity must be greater than 0');
    });
  });

  describe('push', () => {
    it('adds values in order', () => {
      const buf = new RingBuffer<number>(5);
      buf.push(10);
      buf.push(20);
      buf.push(30);
      expect(buf.toArray()).toEqual([10, 20, 30]);
    });

    it('evicts oldest when capacity is exceeded', () => {
      const buf = new RingBuffer<number>(3);
      buf.push(1);
      buf.push(2);
      buf.push(3);
      buf.push(4);
      expect(buf.toArray()).toEqual([2, 3, 4]);
    });

    it('evicts multiple oldest values over time', () => {
      const buf = new RingBuffer<number>(2);
      buf.push(1);
      buf.push(2);
      buf.push(3);
      buf.push(4);
      buf.push(5);
      expect(buf.toArray()).toEqual([4, 5]);
    });
  });

  describe('toArray', () => {
    it('returns a copy of the internal array', () => {
      const buf = new RingBuffer<number>(5);
      buf.push(1);
      buf.push(2);
      const arr = buf.toArray();
      arr.push(99);
      expect(buf.toArray()).toEqual([1, 2]);
    });

    it('returns an empty array when buffer is empty', () => {
      const buf = new RingBuffer<number>(5);
      expect(buf.toArray()).toEqual([]);
    });
  });

  describe('clear', () => {
    it('empties the buffer', () => {
      const buf = new RingBuffer<number>(5);
      buf.push(1);
      buf.push(2);
      buf.clear();
      expect(buf.toArray()).toEqual([]);
    });
  });

  describe('max', () => {
    it('returns null on empty buffer', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      expect(buf.max).toBeNull();
    });

    it('throws without comparator', () => {
      const buf = new RingBuffer<number>(5);
      buf.push(1);
      expect(() => buf.max).toThrow('Comparator required for max()');
    });

    it('returns the maximum value', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      buf.push(3);
      buf.push(1);
      buf.push(5);
      buf.push(2);
      expect(buf.max).toBe(5);
    });

    it('returns the maximum with a single element', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      buf.push(42);
      expect(buf.max).toBe(42);
    });
  });

  describe('min', () => {
    it('returns null on empty buffer', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      expect(buf.min).toBeNull();
    });

    it('throws without comparator', () => {
      const buf = new RingBuffer<number>(5);
      buf.push(1);
      expect(() => buf.min).toThrow('Comparator required for min()');
    });

    it('returns the minimum value', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      buf.push(3);
      buf.push(1);
      buf.push(5);
      buf.push(2);
      expect(buf.min).toBe(1);
    });

    it('returns the minimum with a single element', () => {
      const buf = new RingBuffer<number>(5, (a, b) => a - b);
      buf.push(42);
      expect(buf.min).toBe(42);
    });
  });
});
