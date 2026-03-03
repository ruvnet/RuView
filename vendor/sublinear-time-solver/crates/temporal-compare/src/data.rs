use rand::{rngs::StdRng, SeedableRng, Rng};
use rand_distr::{Normal, Distribution};

#[derive(Clone)]
pub struct Sample {
    pub x: Vec<f32>,  // windowed features
    pub y: f32,       // next value or class
}

pub struct Dataset {
    pub train: Vec<Sample>,
    pub val: Vec<Sample>,
    pub test: Vec<Sample>,
}

/// Synthetic temporal process with regime shifts and delays.
/// Tasks:
/// 1) next_value regression
/// 2) future_bucket classification (coarse future state)
pub fn make_synthetic(window: usize, n: usize, seed: u64) -> Dataset {
    let mut rng = StdRng::seed_from_u64(seed);
    let noise = Normal::new(0.0, 0.3).unwrap();

    // latent regime that flips with low prob
    let mut regime = 0.0f32;
    let mut series: Vec<f32> = Vec::with_capacity(n + window + 10);
    let mut last = 0.0f32;

    for t in 0..(n + window + 10) {
        if rng.gen::<f32>() < 0.02 { regime = if regime == 0.0 { 1.0 } else { 0.0 }; }
        let drift = if regime == 0.0 { 0.02 } else { -0.015 };
        let val = 0.8 * last + drift + noise.sample(&mut rng) as f32;
        series.push(val);
        last = val;
        // occasional delayed impulse
        if t % 37 == 0 { last += 0.9; }
    }

    let mut rows = Vec::new();
    for i in 0..n {
        let w = &series[i..i+window];
        let y = series[i+window];
        let mut x = w.to_vec();
        // add simple time features
        x.push(((i as f32) % 24.0) / 24.0);
        x.push(regime);
        rows.push(Sample { x, y });
    }

    let split1 = (0.7 * rows.len() as f32) as usize;
    let split2 = (0.85 * rows.len() as f32) as usize;
    Dataset {
        train: rows[..split1].to_vec(),
        val: rows[split1..split2].to_vec(),
        test: rows[split2..].to_vec(),
    }
}

pub fn to_class(y: f32) -> usize {
    // 3-bucket classification for coarse future state
    if y < -0.25 { 0 } else if y > 0.25 { 2 } else { 1 }
}