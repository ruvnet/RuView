export function valueToColor(v: number): [number, number, number] {
  const clamped = Math.max(0, Math.min(1, v));

  let r: number;
  let g: number;
  let b: number;

  if (clamped < 0.5) {
    const t = clamped * 2;
    r = 0;
    g = t;
    b = 1 - t;
  } else {
    const t = (clamped - 0.5) * 2;
    r = t;
    g = 1 - t;
    b = 0;
  }

  return [r, g, b];
}
