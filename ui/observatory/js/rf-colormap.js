/**
 * Precomputed scientific colormaps for high-performance RF spectral visualization.
 * Uses 256-entry lookup tables for zero-allocation, 60 FPS pixel pipeline.
 * Complies with ELMAS.md: <= 350 lines, pure deterministic functions.
 */

// Viridis colormap control points (Normalized R, G, B)
const VIRIDIS_POINTS = [
  [0.00, 0x44, 0x01, 0x54],
  [0.13, 0x48, 0x28, 0x78],
  [0.25, 0x3e, 0x4a, 0x89],
  [0.38, 0x31, 0x68, 0x8e],
  [0.50, 0x26, 0x82, 0x8e],
  [0.63, 0x1f, 0x9e, 0x89],
  [0.75, 0x35, 0xb7, 0x79],
  [0.88, 0x6d, 0xcd, 0x59],
  [1.00, 0xfd, 0xe7, 0x25]
];

// Turbo / Thermal RF colormap control points
const TURBO_POINTS = [
  [0.00, 0x1e, 0x29, 0x3b],
  [0.20, 0x02, 0x84, 0xc7],
  [0.40, 0x0d, 0x94, 0x88],
  [0.60, 0x16, 0xa3, 0x4a],
  [0.80, 0xf5, 0x9e, 0x0b],
  [1.00, 0xef, 0x44, 0x44]
];

/**
 * Build 256-entry RGBA lookup table from linear control points.
 * @param {Array} points [[t, r, g, b], ...]
 * @returns {Uint32Array} Packed 32-bit little-endian RGBA values
 */
function buildLut(points) {
  const lut = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    const t = i / 255;
    let p0 = points[0];
    let p1 = points[points.length - 1];

    for (let k = 0; k < points.length - 1; k++) {
      if (t >= points[k][0] && t <= points[k + 1][0]) {
        p0 = points[k];
        p1 = points[k + 1];
        break;
      }
    }

    const span = p1[0] - p0[0] || 1e-6;
    const factor = (t - p0[0]) / span;
    const r = Math.round(p0[1] + (p1[1] - p0[1]) * factor);
    const g = Math.round(p0[2] + (p1[2] - p0[2]) * factor);
    const b = Math.round(p0[3] + (p1[3] - p0[3]) * factor);

    // Little-endian Uint32: AABBGGRR
    lut[i] = (255 << 24) | (b << 16) | (g << 8) | r;
  }
  return lut;
}

export const VIRIDIS_LUT = buildLut(VIRIDIS_POINTS);
export const TURBO_LUT = buildLut(TURBO_POINTS);

/**
 * Map a normalized scalar (0.0 to 1.0) to packed Uint32 RGBA.
 * @param {number} val 
 * @param {Uint32Array} lut 
 * @returns {number} Packed 32-bit color
 */
export function scalarToColor(val, lut = VIRIDIS_LUT) {
  if (isNaN(val)) return lut[0];
  const idx = Math.max(0, Math.min(255, Math.round(val * 255)));
  return lut[idx];
}

