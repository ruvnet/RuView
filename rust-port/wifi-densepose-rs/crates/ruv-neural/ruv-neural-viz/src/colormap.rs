//! Color mapping utilities for brain topology visualization.

/// Maps scalar values in [0, 1] to RGB colors via piecewise-linear interpolation.
#[derive(Debug, Clone)]
pub struct ColorMap {
    /// Sorted color stops: (position, [r, g, b]).
    stops: Vec<(f64, [u8; 3])>,
}

impl ColorMap {
    /// Create a colormap from a list of (position, color) stops.
    ///
    /// Positions must be in ascending order and span at least two values.
    /// Values outside the stop range are clamped.
    pub fn new(stops: Vec<(f64, [u8; 3])>) -> Self {
        assert!(stops.len() >= 2, "ColorMap requires at least two stops");
        Self { stops }
    }

    /// Cool-warm diverging colormap (blue -> white -> red).
    pub fn cool_warm() -> Self {
        Self {
            stops: vec![
                (0.0, [59, 76, 192]),    // blue
                (0.5, [221, 221, 221]),   // near-white
                (1.0, [180, 4, 38]),      // red
            ],
        }
    }

    /// Viridis-like sequential colormap (dark purple -> teal -> yellow).
    pub fn viridis() -> Self {
        Self {
            stops: vec![
                (0.0, [68, 1, 84]),       // dark purple
                (0.25, [59, 82, 139]),     // blue-purple
                (0.5, [33, 145, 140]),     // teal
                (0.75, [94, 201, 98]),     // green
                (1.0, [253, 231, 37]),     // yellow
            ],
        }
    }

    /// Generate distinct colors for brain modules (partitions).
    ///
    /// Uses evenly-spaced hues on the HSV color wheel.
    pub fn module_colors(num_modules: usize) -> Vec<[u8; 3]> {
        if num_modules == 0 {
            return Vec::new();
        }
        (0..num_modules)
            .map(|i| {
                let hue = (i as f64) / (num_modules as f64) * 360.0;
                hsv_to_rgb(hue, 0.7, 0.9)
            })
            .collect()
    }

    /// Map a value in [0, 1] to an RGB color.
    ///
    /// Values outside [0, 1] are clamped.
    pub fn map(&self, value: f64) -> [u8; 3] {
        let v = value.clamp(0.0, 1.0);

        // Before first stop
        if v <= self.stops[0].0 {
            return self.stops[0].1;
        }
        // After last stop
        if v >= self.stops[self.stops.len() - 1].0 {
            return self.stops[self.stops.len() - 1].1;
        }

        // Find the two surrounding stops
        for w in self.stops.windows(2) {
            let (p0, c0) = w[0];
            let (p1, c1) = w[1];
            if v >= p0 && v <= p1 {
                let t = if (p1 - p0).abs() < 1e-12 {
                    0.0
                } else {
                    (v - p0) / (p1 - p0)
                };
                return [
                    lerp_u8(c0[0], c1[0], t),
                    lerp_u8(c0[1], c1[1], t),
                    lerp_u8(c0[2], c1[2], t),
                ];
            }
        }

        // Fallback (should not reach here)
        self.stops[self.stops.len() - 1].1
    }

    /// Map a value to a hex color string (e.g., "#3B4CC0").
    pub fn map_hex(&self, value: f64) -> String {
        let [r, g, b] = self.map(value);
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }
}

/// Linearly interpolate between two u8 values.
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let result = (a as f64) * (1.0 - t) + (b as f64) * t;
    result.round().clamp(0.0, 255.0) as u8
}

/// Convert HSV (h in [0,360], s in [0,1], v in [0,1]) to RGB.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cool_warm_blue_at_zero() {
        let cm = ColorMap::cool_warm();
        let c = cm.map(0.0);
        assert_eq!(c, [59, 76, 192]);
    }

    #[test]
    fn cool_warm_white_at_half() {
        let cm = ColorMap::cool_warm();
        let c = cm.map(0.5);
        assert_eq!(c, [221, 221, 221]);
    }

    #[test]
    fn cool_warm_red_at_one() {
        let cm = ColorMap::cool_warm();
        let c = cm.map(1.0);
        assert_eq!(c, [180, 4, 38]);
    }

    #[test]
    fn map_hex_format() {
        let cm = ColorMap::cool_warm();
        let hex = cm.map_hex(0.0);
        assert_eq!(hex, "#3B4CC0");
    }

    #[test]
    fn module_colors_distinct() {
        let colors = ColorMap::module_colors(5);
        assert_eq!(colors.len(), 5);
        // All colors should be distinct
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "module colors must be distinct");
            }
        }
    }

    #[test]
    fn module_colors_empty() {
        let colors = ColorMap::module_colors(0);
        assert!(colors.is_empty());
    }

    #[test]
    fn clamp_below_zero() {
        let cm = ColorMap::cool_warm();
        let c = cm.map(-0.5);
        assert_eq!(c, cm.map(0.0));
    }

    #[test]
    fn clamp_above_one() {
        let cm = ColorMap::cool_warm();
        let c = cm.map(1.5);
        assert_eq!(c, cm.map(1.0));
    }
}
