use wasm_bindgen::prelude::*;
use web_sys::console;

/// Log a message to the browser console
#[wasm_bindgen]
pub fn log(message: &str) {
    console::log_1(&message.into());
}

/// Log an error to the browser console
#[wasm_bindgen]
pub fn log_error(message: &str) {
    console::error_1(&message.into());
}

/// Compute cosine similarity between two vectors
#[wasm_bindgen]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, JsError> {
    if a.len() != b.len() {
        return Err(JsError::new("Vectors must have same length"));
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(JsError::new("Cannot compute similarity for zero vector"));
    }

    Ok(dot / (norm_a * norm_b))
}

/// Compute L2 norm of a vector
#[wasm_bindgen]
pub fn l2_norm(vec: &[f32]) -> f32 {
    vec.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalize a vector to unit length
#[wasm_bindgen]
pub fn normalize(vec: &mut [f32]) -> Result<(), JsError> {
    let norm = l2_norm(vec);
    if norm == 0.0 {
        return Err(JsError::new("Cannot normalize zero vector"));
    }

    for x in vec.iter_mut() {
        *x /= norm;
    }

    Ok(())
}

/// Compute softmax of a vector
#[wasm_bindgen]
pub fn softmax(vec: &mut [f32]) {
    // Subtract max for numerical stability
    let max = vec.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp and sum
    let mut sum = 0.0;
    for x in vec.iter_mut() {
        *x = (*x - max).exp();
        sum += *x;
    }

    // Normalize
    for x in vec.iter_mut() {
        *x /= sum;
    }
}

/// Compute attention weights from scores
#[wasm_bindgen]
pub fn attention_weights(scores: &mut [f32], temperature: Option<f32>) {
    let temp = temperature.unwrap_or(1.0);

    // Scale by temperature
    for score in scores.iter_mut() {
        *score /= temp;
    }

    // Apply softmax
    softmax(scores);
}

/// Batch normalize vectors
#[wasm_bindgen]
pub fn batch_normalize(vectors: JsValue, epsilon: Option<f32>) -> Result<Vec<f32>, JsError> {
    let eps = epsilon.unwrap_or(1e-8);
    let mut vecs: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(vectors)?;

    if vecs.is_empty() {
        return Ok(Vec::new());
    }

    let dim = vecs[0].len();
    let batch_size = vecs.len();

    // Compute mean
    let mut mean = vec![0.0; dim];
    for vec in &vecs {
        for (i, &val) in vec.iter().enumerate() {
            mean[i] += val;
        }
    }
    for m in &mut mean {
        *m /= batch_size as f32;
    }

    // Compute variance
    let mut variance = vec![0.0; dim];
    for vec in &vecs {
        for (i, &val) in vec.iter().enumerate() {
            let diff = val - mean[i];
            variance[i] += diff * diff;
        }
    }
    for v in &mut variance {
        *v /= batch_size as f32;
    }

    // Normalize
    for vec in &mut vecs {
        for (i, val) in vec.iter_mut().enumerate() {
            *val = (*val - mean[i]) / (variance[i] + eps).sqrt();
        }
    }

    Ok(vecs.into_iter().flatten().collect())
}

/// Generate random orthogonal matrix (for initialization)
#[wasm_bindgen]
pub fn random_orthogonal_matrix(dim: usize) -> Vec<f32> {
    use js_sys::Math;

    let mut matrix = vec![0.0; dim * dim];

    // Generate random matrix
    for i in 0..dim {
        for j in 0..dim {
            matrix[i * dim + j] = (Math::random() as f32 - 0.5) * 2.0;
        }
    }

    // QR decomposition (simplified Gram-Schmidt)
    for i in 0..dim {
        // Normalize column i
        let mut norm = 0.0;
        for j in 0..dim {
            let val = matrix[j * dim + i];
            norm += val * val;
        }
        norm = norm.sqrt();

        for j in 0..dim {
            matrix[j * dim + i] /= norm;
        }

        // Orthogonalize remaining columns
        for k in (i + 1)..dim {
            let mut dot = 0.0;
            for j in 0..dim {
                dot += matrix[j * dim + i] * matrix[j * dim + k];
            }

            for j in 0..dim {
                matrix[j * dim + k] -= dot * matrix[j * dim + i];
            }
        }
    }

    matrix
}

/// Compute pairwise distances between vectors
#[wasm_bindgen]
pub fn pairwise_distances(vectors: JsValue) -> Result<Vec<f32>, JsError> {
    let vecs: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(vectors)?;
    let n = vecs.len();
    let mut distances = vec![0.0; n * n];

    for i in 0..n {
        for j in 0..n {
            if i == j {
                distances[i * n + j] = 0.0;
            } else {
                let mut dist = 0.0;
                for k in 0..vecs[i].len() {
                    let diff = vecs[i][k] - vecs[j][k];
                    dist += diff * diff;
                }
                distances[i * n + j] = dist.sqrt();
            }
        }
    }

    Ok(distances)
}
