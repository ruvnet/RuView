#version 300 es
//! Matrix Multiplication Fragment Shader
//!
//! Computes C = A * B using texture-based GPU compute.
//!
//! ## Usage
//!
//! - A and B are R32F textures (single-channel float)
//! - Output is rendered to framebuffer-attached texture
//! - Each fragment computes one element of C
//!
//! ## Texture Layout
//!
//! - A: rows = M, cols = K (stored row-major)
//! - B: rows = K, cols = N (stored row-major)
//! - C: rows = M, cols = N (output)
//!
//! ## Performance Notes
//!
//! - Use texture size that's power of 2 for best performance
//! - NEAREST filtering required for exact texel fetch
//! - Loop unrolling may help on some GPUs

precision highp float;

// Input matrices as textures
uniform sampler2D u_A;
uniform sampler2D u_B;

// Matrix dimensions: (M, K, N)
// A is MxK, B is KxN, C is MxN
uniform vec3 u_dims;

// Texture coordinates from vertex shader
in vec2 v_texcoord;

// Output value (single float stored in R channel)
out float fragColor;

void main() {
    float M = u_dims.x;
    float K = u_dims.y;
    float N = u_dims.z;

    // Calculate output position (row i, column j)
    // v_texcoord is normalized [0,1], so we scale to pixel coordinates
    float i = floor(v_texcoord.y * M);
    float j = floor(v_texcoord.x * N);

    // Bounds check (fragments outside valid range output 0)
    if (i >= M || j >= N) {
        fragColor = 0.0;
        return;
    }

    // Compute dot product of row i of A with column j of B
    float sum = 0.0;

    // Manual loop unrolling for common case (K <= 4)
    // This helps on mobile GPUs with limited loop support
    #if defined(UNROLL_4)
    if (K <= 4.0) {
        if (K >= 1.0) {
            float a0 = texture(u_A, vec2(0.5 / K, (i + 0.5) / M)).r;
            float b0 = texture(u_B, vec2((j + 0.5) / N, 0.5 / K)).r;
            sum += a0 * b0;
        }
        if (K >= 2.0) {
            float a1 = texture(u_A, vec2(1.5 / K, (i + 0.5) / M)).r;
            float b1 = texture(u_B, vec2((j + 0.5) / N, 1.5 / K)).r;
            sum += a1 * b1;
        }
        if (K >= 3.0) {
            float a2 = texture(u_A, vec2(2.5 / K, (i + 0.5) / M)).r;
            float b2 = texture(u_B, vec2((j + 0.5) / N, 2.5 / K)).r;
            sum += a2 * b2;
        }
        if (K >= 4.0) {
            float a3 = texture(u_A, vec2(3.5 / K, (i + 0.5) / M)).r;
            float b3 = texture(u_B, vec2((j + 0.5) / N, 3.5 / K)).r;
            sum += a3 * b3;
        }
    } else
    #endif
    {
        // General loop for arbitrary K
        // We add 0.5 to center the sample within each texel
        for (float k = 0.0; k < K; k += 1.0) {
            // Sample A[i, k] - row i, column k
            // Texture coordinate: x = (k + 0.5) / K, y = (i + 0.5) / M
            float a_val = texture(u_A, vec2((k + 0.5) / K, (i + 0.5) / M)).r;

            // Sample B[k, j] - row k, column j
            // Texture coordinate: x = (j + 0.5) / N, y = (k + 0.5) / K
            float b_val = texture(u_B, vec2((j + 0.5) / N, (k + 0.5) / K)).r;

            sum += a_val * b_val;
        }
    }

    fragColor = sum;
}
