import { valueToColor } from '@/utils/colorMap';

describe('valueToColor', () => {
  it('returns blue at 0', () => {
    const [r, g, b] = valueToColor(0);
    expect(r).toBe(0);
    expect(g).toBe(0);
    expect(b).toBe(1);
  });

  it('returns green at 0.5', () => {
    const [r, g, b] = valueToColor(0.5);
    expect(r).toBe(0);
    expect(g).toBe(1);
    expect(b).toBe(0);
  });

  it('returns red at 1', () => {
    const [r, g, b] = valueToColor(1);
    expect(r).toBe(1);
    expect(g).toBe(0);
    expect(b).toBe(0);
  });

  it('clamps values below 0 to the same as 0', () => {
    const [r, g, b] = valueToColor(-0.5);
    const [r0, g0, b0] = valueToColor(0);
    expect(r).toBe(r0);
    expect(g).toBe(g0);
    expect(b).toBe(b0);
  });

  it('clamps values above 1 to the same as 1', () => {
    const [r, g, b] = valueToColor(1.5);
    const [r1, g1, b1] = valueToColor(1);
    expect(r).toBe(r1);
    expect(g).toBe(g1);
    expect(b).toBe(b1);
  });

  it('interpolates between blue and green for 0.25', () => {
    const [r, g, b] = valueToColor(0.25);
    expect(r).toBe(0);
    expect(g).toBeCloseTo(0.5);
    expect(b).toBeCloseTo(0.5);
  });

  it('interpolates between green and red for 0.75', () => {
    const [r, g, b] = valueToColor(0.75);
    expect(r).toBeCloseTo(0.5);
    expect(g).toBeCloseTo(0.5);
    expect(b).toBe(0);
  });

  it('returns a 3-element tuple', () => {
    const result = valueToColor(0.5);
    expect(result).toHaveLength(3);
  });

  it('all channels are in [0, 1] range for edge values', () => {
    for (const v of [-1, 0, 0.1, 0.5, 0.9, 1, 2]) {
      const [r, g, b] = valueToColor(v);
      expect(r).toBeGreaterThanOrEqual(0);
      expect(r).toBeLessThanOrEqual(1);
      expect(g).toBeGreaterThanOrEqual(0);
      expect(g).toBeLessThanOrEqual(1);
      expect(b).toBeGreaterThanOrEqual(0);
      expect(b).toBeLessThanOrEqual(1);
    }
  });
});
