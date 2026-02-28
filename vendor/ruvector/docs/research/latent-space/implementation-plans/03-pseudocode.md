# SPARC Pseudocode: RuVector Attention Mechanisms

## Executive Summary

This document provides comprehensive pseudocode for all attention mechanisms proposed for the RuVector GNN latent-graph interplay system. Following SPARC methodology, this serves as the bridge between specification (requirements) and architecture (system design).

**Scope**: Complete algorithmic specifications for attention mechanisms, training procedures, and optimization strategies.

**Target Audience**: Implementers who will translate these algorithms into Rust code.

**Conventions**:
- UPPERCASE: Algorithm names, constants
- lowercase: Variables, parameters
- `←`: Assignment
- `∈`: Set membership
- Arrays are 0-indexed unless specified
- All complexity analysis uses Big-O notation

---

## Table of Contents

1. [Core Attention Mechanisms](#1-core-attention-mechanisms)
2. [Geometric Attention](#2-geometric-attention)
3. [Sparse Attention](#3-sparse-attention)
4. [Graph Attention](#4-graph-attention)
5. [Adaptive Attention](#5-adaptive-attention)
6. [Training Procedures](#6-training-procedures)
7. [Data Structures](#7-data-structures)
8. [Complexity Summary](#8-complexity-summary)

---

## 1. Core Attention Mechanisms

### 1.1 Scaled Dot-Product Attention

**Purpose**: Foundation attention mechanism for all variants

**Complexity**:
- Time: O(n·d²) where n = number of keys, d = embedding dimension
- Space: O(n)

```
ALGORITHM: ScaledDotProductAttention
INPUT:
    Q: query vector [d]
    K: key matrix [n × d]
    V: value matrix [n × d]
    d_k: key dimension (scalar)
OUTPUT:
    output: attention output [d]
    weights: attention weights [n]

BEGIN
    // 1. Compute attention scores
    scores ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        scores[i] ← DotProduct(Q, K[i]) / sqrt(d_k)
    END FOR

    // 2. Apply softmax for normalization
    weights ← Softmax(scores)

    // 3. Weighted sum of values
    output ← ZeroVector(d)
    FOR i ← 0 TO n-1 DO
        output ← output + weights[i] * V[i]
    END FOR

    RETURN output, weights
END

SUBROUTINE: DotProduct
INPUT: x[d], y[d]
OUTPUT: scalar
BEGIN
    sum ← 0
    FOR i ← 0 TO d-1 DO
        sum ← sum + x[i] * y[i]
    END FOR
    RETURN sum
END

SUBROUTINE: Softmax
INPUT: scores[n]
OUTPUT: probabilities[n]
BEGIN
    // Numerical stability: subtract max
    max_score ← Max(scores)

    exp_scores ← EMPTY_ARRAY[n]
    sum_exp ← 0

    FOR i ← 0 TO n-1 DO
        exp_scores[i] ← exp(scores[i] - max_score)
        sum_exp ← sum_exp + exp_scores[i]
    END FOR

    probabilities ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        probabilities[i] ← exp_scores[i] / sum_exp
    END FOR

    RETURN probabilities
END
```

---

### 1.2 Multi-Head Attention

**Purpose**: Learn multiple representation subspaces simultaneously

**Complexity**:
- Time: O(h·n·d²/h²) = O(n·d²/h) where h = number of heads
- Space: O(h·d)

```
ALGORITHM: MultiHeadAttention
INPUT:
    Q: query vector [d_model]
    K: key matrix [n × d_model]
    V: value matrix [n × d_model]
    num_heads: number of attention heads
    W_Q: query projection weights [num_heads × d_head × d_model]
    W_K: key projection weights [num_heads × d_head × d_model]
    W_V: value projection weights [num_heads × d_head × d_model]
    W_O: output projection weights [d_model × d_model]
OUTPUT:
    output: multi-head attention output [d_model]

CONSTANTS:
    d_head ← d_model / num_heads

BEGIN
    heads ← EMPTY_ARRAY[num_heads]

    // 1. Project and compute attention for each head
    FOR h ← 0 TO num_heads-1 DO
        // Project query
        Q_h ← LinearTransform(Q, W_Q[h])  // [d_head]

        // Project keys
        K_h ← EMPTY_MATRIX[n × d_head]
        FOR i ← 0 TO n-1 DO
            K_h[i] ← LinearTransform(K[i], W_K[h])
        END FOR

        // Project values
        V_h ← EMPTY_MATRIX[n × d_head]
        FOR i ← 0 TO n-1 DO
            V_h[i] ← LinearTransform(V[i], W_V[h])
        END FOR

        // Compute attention for this head
        head_output, _ ← ScaledDotProductAttention(Q_h, K_h, V_h, d_head)
        heads[h] ← head_output
    END FOR

    // 2. Concatenate all heads
    concat ← Concatenate(heads[0], heads[1], ..., heads[num_heads-1])

    // 3. Final linear projection
    output ← LinearTransform(concat, W_O)

    RETURN output
END

SUBROUTINE: LinearTransform
INPUT: x[d_in], W[d_out × d_in]
OUTPUT: y[d_out]
BEGIN
    y ← ZeroVector(d_out)
    FOR i ← 0 TO d_out-1 DO
        FOR j ← 0 TO d_in-1 DO
            y[i] ← y[i] + W[i][j] * x[j]
        END FOR
    END FOR
    RETURN y
END

SUBROUTINE: Concatenate
INPUT: vectors... (variable number of vectors)
OUTPUT: concatenated vector
BEGIN
    total_dim ← Sum of all input dimensions
    result ← EMPTY_ARRAY[total_dim]
    offset ← 0

    FOR EACH vector IN vectors DO
        FOR i ← 0 TO Length(vector)-1 DO
            result[offset + i] ← vector[i]
        END FOR
        offset ← offset + Length(vector)
    END FOR

    RETURN result
END
```

---

## 2. Geometric Attention

### 2.1 Hyperbolic Attention (Poincaré Ball Model)

**Purpose**: Capture hierarchical structure using hyperbolic geometry

**Complexity**:
- Time: O(n·d²) (same as Euclidean, but with more expensive ops)
- Space: O(n)

**Geometric Background**:
```
Poincaré Ball: B^d = {x ∈ R^d : ||x|| < 1}
Distance: d_P(x,y) = arcosh(1 + 2||x-y||²/((1-||x||²)(1-||y||²)))
```

```
ALGORITHM: HyperbolicAttention
INPUT:
    query: query point in Poincaré ball [d]
    keys: key points in Poincaré ball [n × d]
    values: value points in Poincaré ball [n × d]
    curvature: negative curvature (typically -1.0)
    temperature: softmax temperature
OUTPUT:
    output: aggregated point in Poincaré ball [d]

BEGIN
    // 1. Compute hyperbolic distances as similarity scores
    scores ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        // Negative distance = similarity (closer = higher score)
        scores[i] ← -PoincareDistance(query, keys[i], curvature)
    END FOR

    // 2. Softmax to get attention weights
    weights ← Softmax(scores / temperature)

    // 3. Hyperbolic weighted aggregation using Möbius addition
    result ← ZeroVector(d)  // Origin in Poincaré ball

    FOR i ← 0 TO n-1 DO
        // Scale value by weight using Möbius scalar multiplication
        scaled_value ← MobiusScalarMult(weights[i], values[i], curvature)

        // Add to result using Möbius addition
        result ← MobiusAdd(result, scaled_value, curvature)
    END FOR

    RETURN result
END

SUBROUTINE: PoincareDistance
INPUT: x[d], y[d], curvature
OUTPUT: distance (scalar)
BEGIN
    // Compute squared norms
    x_norm_sq ← L2NormSquared(x)
    y_norm_sq ← L2NormSquared(y)

    // Ensure points are inside the ball (||x|| < 1, ||y|| < 1)
    IF x_norm_sq >= 1.0 OR y_norm_sq >= 1.0 THEN
        ERROR "Points must be inside Poincaré ball"
    END IF

    // Compute squared distance between points
    diff ← Subtract(x, y)
    diff_norm_sq ← L2NormSquared(diff)

    // Poincaré distance formula
    numerator ← 2.0 * diff_norm_sq
    denominator ← (1.0 - x_norm_sq) * (1.0 - y_norm_sq)

    arg ← 1.0 + numerator / denominator

    // Numerical stability: clamp arg >= 1.0
    IF arg < 1.0 THEN
        arg ← 1.0
    END IF

    distance ← sqrt(abs(curvature)) * arcosh(arg)

    RETURN distance
END

SUBROUTINE: MobiusAdd
INPUT: x[d], y[d], curvature
OUTPUT: z[d] (Möbius sum x ⊕ y)
BEGIN
    // Special case: if x is origin, return y
    IF IsZero(x) THEN
        RETURN y
    END IF

    // Special case: if y is origin, return x
    IF IsZero(y) THEN
        RETURN x
    END IF

    // Compute norms and dot product
    x_norm_sq ← L2NormSquared(x)
    y_norm_sq ← L2NormSquared(y)
    xy_dot ← DotProduct(x, y)

    // Möbius addition formula:
    // z = ((1 + 2c⟨x,y⟩ + c||y||²)x + (1 - c||x||²)y) /
    //     (1 + 2c⟨x,y⟩ + c²||x||²||y||²)

    c ← -curvature  // For Poincaré ball, typically c = 1

    numerator_x_coef ← 1.0 + 2.0*c*xy_dot + c*y_norm_sq
    numerator_y_coef ← 1.0 - c*x_norm_sq
    denominator ← 1.0 + 2.0*c*xy_dot + c*c*x_norm_sq*y_norm_sq

    numerator ← Add(
        Scale(x, numerator_x_coef),
        Scale(y, numerator_y_coef)
    )

    z ← Scale(numerator, 1.0 / denominator)

    // Project back to ball if numerical errors pushed outside
    z_norm ← L2Norm(z)
    IF z_norm >= 1.0 THEN
        z ← Scale(z, 0.99 / z_norm)  // Project to ball with margin
    END IF

    RETURN z
END

SUBROUTINE: MobiusScalarMult
INPUT: r (scalar), x[d], curvature
OUTPUT: r ⊗ x (Möbius scalar multiplication)
BEGIN
    // Handle special cases
    IF r == 0 OR IsZero(x) THEN
        RETURN ZeroVector(d)
    END IF

    x_norm ← L2Norm(x)
    c ← -curvature

    // Möbius scalar multiplication:
    // r ⊗ x = (1/√c) * tanh(r * arctanh(√c * ||x||)) * (x / ||x||)

    sqrt_c ← sqrt(c)
    arctanh_arg ← sqrt_c * x_norm

    // Numerical stability
    IF arctanh_arg >= 1.0 THEN
        arctanh_arg ← 0.999
    END IF

    arctanh_val ← arctanh(arctanh_arg)
    tanh_arg ← r * arctanh_val
    tanh_val ← tanh(tanh_arg)

    scale_factor ← (1.0 / sqrt_c) * tanh_val / x_norm

    result ← Scale(x, scale_factor)

    RETURN result
END

SUBROUTINE: L2NormSquared
INPUT: x[d]
OUTPUT: ||x||² (scalar)
BEGIN
    sum ← 0
    FOR i ← 0 TO d-1 DO
        sum ← sum + x[i] * x[i]
    END FOR
    RETURN sum
END

SUBROUTINE: L2Norm
INPUT: x[d]
OUTPUT: ||x|| (scalar)
BEGIN
    RETURN sqrt(L2NormSquared(x))
END

SUBROUTINE: Subtract
INPUT: x[d], y[d]
OUTPUT: x - y [d]
BEGIN
    result ← EMPTY_ARRAY[d]
    FOR i ← 0 TO d-1 DO
        result[i] ← x[i] - y[i]
    END FOR
    RETURN result
END

SUBROUTINE: Add
INPUT: x[d], y[d]
OUTPUT: x + y [d]
BEGIN
    result ← EMPTY_ARRAY[d]
    FOR i ← 0 TO d-1 DO
        result[i] ← x[i] + y[i]
    END FOR
    RETURN result
END

SUBROUTINE: Scale
INPUT: x[d], scalar
OUTPUT: scalar * x [d]
BEGIN
    result ← EMPTY_ARRAY[d]
    FOR i ← 0 TO d-1 DO
        result[i] ← scalar * x[i]
    END FOR
    RETURN result
END

SUBROUTINE: IsZero
INPUT: x[d]
OUTPUT: boolean
BEGIN
    epsilon ← 1e-10
    RETURN L2Norm(x) < epsilon
END
```

---

## 3. Sparse Attention

### 3.1 Local + Global Sparse Attention

**Purpose**: Reduce O(n²) to O(k_local + k_global) for large graphs

**Complexity**:
- Time: O(k_local·d + k_global·d) where k_local, k_global << n
- Space: O(k_local + k_global)

```
ALGORITHM: SparseLocalGlobalAttention
INPUT:
    query: query vector [d]
    all_neighbors: all neighbor embeddings [n × d]
    neighbor_layers: HNSW layer for each neighbor [n]
    local_window: size of local neighborhood
    global_indices: indices of global attention nodes
OUTPUT:
    output: attention output [d]

BEGIN
    // 1. Partition neighbors into local and global
    local_neighbors ← EMPTY_LIST
    local_indices ← EMPTY_LIST
    global_neighbors ← EMPTY_LIST
    global_indices_actual ← EMPTY_LIST

    FOR i ← 0 TO n-1 DO
        IF neighbor_layers[i] == 0 AND Length(local_neighbors) < local_window THEN
            // Layer 0 = local neighbors
            local_neighbors.Append(all_neighbors[i])
            local_indices.Append(i)
        ELSE IF neighbor_layers[i] > 0 AND i IN global_indices THEN
            // Higher layers = global neighbors
            global_neighbors.Append(all_neighbors[i])
            global_indices_actual.Append(i)
        END IF
    END FOR

    // 2. Compute local attention
    local_output ← ZeroVector(d)
    IF Length(local_neighbors) > 0 THEN
        local_K ← ConvertToMatrix(local_neighbors)
        local_V ← local_K  // Self-attention
        local_output, _ ← ScaledDotProductAttention(
            query, local_K, local_V, d
        )
    END IF

    // 3. Compute global attention
    global_output ← ZeroVector(d)
    IF Length(global_neighbors) > 0 THEN
        global_K ← ConvertToMatrix(global_neighbors)
        global_V ← global_K
        global_output, _ ← ScaledDotProductAttention(
            query, global_K, global_V, d
        )
    END IF

    // 4. Learned gating to combine local and global
    alpha ← LearnedGate(query, local_output, global_output)

    // 5. Combine outputs
    output ← ZeroVector(d)
    FOR i ← 0 TO d-1 DO
        output[i] ← alpha * local_output[i] + (1.0 - alpha) * global_output[i]
    END FOR

    RETURN output
END

SUBROUTINE: LearnedGate
INPUT: query[d], local_output[d], global_output[d]
OUTPUT: alpha (scalar in [0, 1])
BEGIN
    // Concatenate all inputs
    concat ← Concatenate(query, local_output, global_output)

    // Linear projection + sigmoid
    gate_weights ← LEARNED_PARAMETERS[3*d]  // Learned during training
    bias ← LEARNED_BIAS  // Learned during training

    logit ← DotProduct(concat, gate_weights) + bias
    alpha ← Sigmoid(logit)

    RETURN alpha
END

SUBROUTINE: Sigmoid
INPUT: x (scalar)
OUTPUT: sigmoid(x) in [0, 1]
BEGIN
    RETURN 1.0 / (1.0 + exp(-x))
END
```

---

### 3.2 Linear Attention (Performer / Random Features)

**Purpose**: O(n·d) complexity using kernel approximation

**Complexity**:
- Time: O(n·D·d) where D = number of random features
- Space: O(D·d)

```
ALGORITHM: LinearAttention
INPUT:
    query: query vector [d]
    keys: key matrix [n × d]
    values: value matrix [n × d]
    num_features: number of random features D
    random_matrix: random projection matrix [D × d]
OUTPUT:
    output: attention output [d]

BEGIN
    // 1. Apply feature map to query
    phi_Q ← FeatureMap(query, random_matrix, num_features)

    // 2. Apply feature map to all keys
    phi_K ← EMPTY_MATRIX[n × num_features]
    FOR i ← 0 TO n-1 DO
        phi_K[i] ← FeatureMap(keys[i], random_matrix, num_features)
    END FOR

    // 3. Compute K^T V (sum over neighbors) - O(n·D·d)
    KV_sum ← ZeroMatrix(num_features, d)
    FOR i ← 0 TO n-1 DO
        FOR j ← 0 TO num_features-1 DO
            FOR k ← 0 TO d-1 DO
                KV_sum[j][k] ← KV_sum[j][k] + phi_K[i][j] * values[i][k]
            END FOR
        END FOR
    END FOR

    // 4. Compute Q·(K^T V) - O(D·d)
    numerator ← ZeroVector(d)
    FOR k ← 0 TO d-1 DO
        FOR j ← 0 TO num_features-1 DO
            numerator[k] ← numerator[k] + phi_Q[j] * KV_sum[j][k]
        END FOR
    END FOR

    // 5. Compute K^T 1 (sum of feature-mapped keys) - O(n·D)
    K_sum ← ZeroVector(num_features)
    FOR i ← 0 TO n-1 DO
        FOR j ← 0 TO num_features-1 DO
            K_sum[j] ← K_sum[j] + phi_K[i][j]
        END FOR
    END FOR

    // 6. Compute denominator Q·(K^T 1) - O(D)
    denominator ← DotProduct(phi_Q, K_sum)

    // 7. Normalize
    output ← Scale(numerator, 1.0 / (denominator + 1e-10))

    RETURN output
END

SUBROUTINE: FeatureMap
INPUT: x[d], random_matrix[D × d], num_features D
OUTPUT: features[D]
BEGIN
    // Random Fourier Features
    // φ(x) = sqrt(1/D) * [cos(w₁·x), sin(w₁·x), cos(w₂·x), sin(w₂·x), ...]

    scale ← 1.0 / sqrt(num_features)
    features ← EMPTY_ARRAY[num_features]

    FOR i ← 0 TO num_features/2 - 1 DO
        // Get random projection
        w ← random_matrix[i]
        projection ← DotProduct(w, x)

        // Apply cos and sin
        features[2*i] ← scale * cos(projection)
        features[2*i + 1] ← scale * sin(projection)
    END FOR

    RETURN features
END

SUBROUTINE: ZeroMatrix
INPUT: rows, cols
OUTPUT: matrix[rows × cols]
BEGIN
    matrix ← EMPTY_MATRIX[rows × cols]
    FOR i ← 0 TO rows-1 DO
        FOR j ← 0 TO cols-1 DO
            matrix[i][j] ← 0.0
        END FOR
    END FOR
    RETURN matrix
END
```

---

### 3.3 Flash Attention (Tiled / Memory-Efficient)

**Purpose**: O(n) memory instead of O(n²) through tiling

**Complexity**:
- Time: O(n²·d) (same as standard, but better cache locality)
- Space: O(n) instead of O(n²)

```
ALGORITHM: FlashAttention
INPUT:
    query: query vector [d]
    keys: key matrix [n × d]
    values: value matrix [n × d]
    block_size: tile size B (typically 64-128)
OUTPUT:
    output: attention output [d]

BEGIN
    n ← Length(keys)
    output ← ZeroVector(d)
    row_max ← -INFINITY
    row_sum ← 0.0

    num_blocks ← Ceiling(n / block_size)

    // Process keys/values in blocks (tiles)
    FOR block_idx ← 0 TO num_blocks-1 DO
        // 1. Define current block range
        chunk_start ← block_idx * block_size
        chunk_end ← Min(chunk_start + block_size, n)
        chunk_size ← chunk_end - chunk_start

        // 2. Extract block of keys and values
        chunk_K ← keys[chunk_start : chunk_end]
        chunk_V ← values[chunk_start : chunk_end]

        // 3. Compute attention scores for this block
        scores ← EMPTY_ARRAY[chunk_size]
        FOR i ← 0 TO chunk_size-1 DO
            scores[i] ← DotProduct(query, chunk_K[i]) / sqrt(d)
        END FOR

        // 4. Online softmax: update running max
        new_max ← Max(row_max, Max(scores))

        // 5. Compute exponentials with new max
        exp_scores ← EMPTY_ARRAY[chunk_size]
        FOR i ← 0 TO chunk_size-1 DO
            exp_scores[i] ← exp(scores[i] - new_max)
        END FOR

        // 6. Correction factor for previous blocks
        correction ← exp(row_max - new_max)

        // 7. Update running sum of exponentials
        chunk_sum ← Sum(exp_scores)
        row_sum ← row_sum * correction + chunk_sum

        // 8. Update running max
        row_max ← new_max

        // 9. Accumulate weighted values with correction
        FOR i ← 0 TO d-1 DO
            output[i] ← output[i] * correction
        END FOR

        FOR i ← 0 TO chunk_size-1 DO
            FOR j ← 0 TO d-1 DO
                output[j] ← output[j] + exp_scores[i] * chunk_V[i][j]
            END FOR
        END FOR
    END FOR

    // 10. Final normalization
    FOR i ← 0 TO d-1 DO
        output[i] ← output[i] / row_sum
    END FOR

    RETURN output
END

SUBROUTINE: Max
INPUT: array[n] OR two scalars
OUTPUT: maximum value
BEGIN
    IF array is provided THEN
        max_val ← array[0]
        FOR i ← 1 TO Length(array)-1 DO
            IF array[i] > max_val THEN
                max_val ← array[i]
            END IF
        END FOR
        RETURN max_val
    ELSE
        // Two scalars
        RETURN IF (a > b) THEN a ELSE b
    END IF
END

SUBROUTINE: Sum
INPUT: array[n]
OUTPUT: sum of elements
BEGIN
    total ← 0
    FOR i ← 0 TO Length(array)-1 DO
        total ← total + array[i]
    END FOR
    RETURN total
END

SUBROUTINE: Ceiling
INPUT: x (real number)
OUTPUT: ⌈x⌉ (smallest integer >= x)
BEGIN
    RETURN integer ceiling of x
END

SUBROUTINE: Min
INPUT: a, b (scalars)
OUTPUT: minimum value
BEGIN
    RETURN IF (a < b) THEN a ELSE b
END
```

---

## 4. Graph Attention

### 4.1 Edge-Featured Attention

**Purpose**: Incorporate edge attributes into attention computation

**Complexity**:
- Time: O(n·(d² + d_edge·d))
- Space: O(n)

```
ALGORITHM: EdgeFeaturedAttention
INPUT:
    query: query node embedding [d]
    keys: neighbor node embeddings [n × d]
    values: neighbor node embeddings [n × d]
    edge_features: edge attributes [n × d_edge]
    W_node: node transformation matrix [d × d]
    W_edge: edge transformation matrix [d_edge × d_attn]
    a: attention coefficient vector [2d + d_attn]
OUTPUT:
    output: aggregated embedding [d]

BEGIN
    // 1. Transform query
    q_trans ← MatrixVectorMult(W_node, query)

    // 2. Transform all keys and edge features
    k_trans ← EMPTY_MATRIX[n × d]
    e_trans ← EMPTY_MATRIX[n × d_attn]

    FOR i ← 0 TO n-1 DO
        k_trans[i] ← MatrixVectorMult(W_node, keys[i])
        e_trans[i] ← MatrixVectorMult(W_edge, edge_features[i])
    END FOR

    // 3. Compute attention scores with edge features
    scores ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        // Concatenate [query || key || edge]
        concat ← Concatenate(q_trans, k_trans[i], e_trans[i])

        // Attention coefficient
        score ← DotProduct(a, concat)

        // Activation (LeakyReLU)
        scores[i] ← LeakyReLU(score, alpha=0.2)
    END FOR

    // 4. Softmax normalization
    weights ← Softmax(scores)

    // 5. Weighted aggregation
    output ← WeightedSum(values, weights)

    RETURN output
END

SUBROUTINE: MatrixVectorMult
INPUT: M[m × n], v[n]
OUTPUT: result[m]
BEGIN
    result ← ZeroVector(m)
    FOR i ← 0 TO m-1 DO
        FOR j ← 0 TO n-1 DO
            result[i] ← result[i] + M[i][j] * v[j]
        END FOR
    END FOR
    RETURN result
END

SUBROUTINE: LeakyReLU
INPUT: x (scalar), alpha (negative slope)
OUTPUT: activated value
BEGIN
    IF x >= 0 THEN
        RETURN x
    ELSE
        RETURN alpha * x
    END IF
END

SUBROUTINE: WeightedSum
INPUT: vectors[n × d], weights[n]
OUTPUT: result[d]
BEGIN
    result ← ZeroVector(d)
    FOR i ← 0 TO n-1 DO
        FOR j ← 0 TO d-1 DO
            result[j] ← result[j] + weights[i] * vectors[i][j]
        END FOR
    END FOR
    RETURN result
END
```

---

### 4.2 RoPE Graph Attention

**Purpose**: Encode graph distances via rotary position embeddings

**Complexity**:
- Time: O(n·d²)
- Space: O(n)

```
ALGORITHM: RoPEGraphAttention
INPUT:
    query: query node embedding [d]
    keys: neighbor node embeddings [n × d]
    values: neighbor node embeddings [n × d]
    distances: graph distances to neighbors [n]
    base: RoPE frequency base (default 10000)
OUTPUT:
    output: attention output [d]

BEGIN
    // 1. Apply RoPE rotation to query (at origin, distance = 0)
    Q_rotated ← ApplyRotation(query, distance=0.0, base)

    // 2. Apply RoPE rotation to keys based on their distances
    K_rotated ← EMPTY_MATRIX[n × d]
    FOR i ← 0 TO n-1 DO
        K_rotated[i] ← ApplyRotation(keys[i], distances[i], base)
    END FOR

    // 3. Compute attention scores with rotated embeddings
    scores ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        scores[i] ← DotProduct(Q_rotated, K_rotated[i])
    END FOR

    // 4. Softmax and aggregate
    weights ← Softmax(scores)
    output ← WeightedSum(values, weights)

    RETURN output
END

SUBROUTINE: ApplyRotation
INPUT: embedding[d], distance (scalar), base
OUTPUT: rotated[d]
BEGIN
    rotated ← ZeroVector(d)

    // Apply rotation to pairs of dimensions
    FOR i ← 0 TO d/2 - 1 DO
        // Compute rotation angle for this dimension pair
        theta ← distance / (base ^ (2.0 * i / d))

        cos_theta ← cos(theta)
        sin_theta ← sin(theta)

        // Rotate dimensions (2*i, 2*i+1)
        rotated[2*i] ← embedding[2*i] * cos_theta - embedding[2*i+1] * sin_theta
        rotated[2*i+1] ← embedding[2*i] * sin_theta + embedding[2*i+1] * cos_theta
    END FOR

    RETURN rotated
END
```

---

### 4.3 Cross-Space (Dual) Attention

**Purpose**: Bridge graph topology and latent space semantics

**Complexity**:
- Time: O(n_graph·d² + k_latent·d² + k_latent²·d)
- Space: O(n_graph + k_latent)

```
ALGORITHM: DualSpaceAttention
INPUT:
    query: query node embedding [d]
    graph_neighbors: topological neighbors [n_graph × d]
    all_embeddings: all node embeddings for latent search [N × d]
    k_latent: number of latent neighbors
OUTPUT:
    output: fused embedding [d]

BEGIN
    // 1. Graph attention (topology-based)
    graph_output, _ ← MultiHeadAttention(
        query,
        graph_neighbors,
        graph_neighbors,
        num_heads=8
    )

    // 2. Find latent neighbors (similarity-based)
    latent_neighbors ← FindTopKSimilar(query, all_embeddings, k_latent)

    // 3. Latent attention (embedding-based)
    latent_output, _ ← MultiHeadAttention(
        query,
        latent_neighbors,
        latent_neighbors,
        num_heads=8
    )

    // 4. Cross-attention (graph context queries latent space)
    cross_output, _ ← MultiHeadAttention(
        graph_output,  // Use graph output as query
        latent_neighbors,
        latent_neighbors,
        num_heads=8
    )

    // 5. Fusion of all three outputs
    concatenated ← Concatenate(graph_output, latent_output, cross_output)

    // 6. Final projection
    W_fusion ← LEARNED_WEIGHTS[d × 3d]
    output ← MatrixVectorMult(W_fusion, concatenated)

    RETURN output
END

SUBROUTINE: FindTopKSimilar
INPUT: query[d], all_embeddings[N × d], k
OUTPUT: top_k_embeddings[k × d]
BEGIN
    similarities ← EMPTY_ARRAY[N]

    // 1. Compute cosine similarity to all embeddings
    FOR i ← 0 TO N-1 DO
        similarities[i] ← CosineSimilarity(query, all_embeddings[i])
    END FOR

    // 2. Find top-k indices
    top_k_indices ← TopKIndices(similarities, k)

    // 3. Extract top-k embeddings
    top_k_embeddings ← EMPTY_MATRIX[k × d]
    FOR i ← 0 TO k-1 DO
        top_k_embeddings[i] ← all_embeddings[top_k_indices[i]]
    END FOR

    RETURN top_k_embeddings
END

SUBROUTINE: CosineSimilarity
INPUT: x[d], y[d]
OUTPUT: similarity in [-1, 1]
BEGIN
    dot ← DotProduct(x, y)
    norm_x ← L2Norm(x)
    norm_y ← L2Norm(y)

    // Avoid division by zero
    IF norm_x == 0 OR norm_y == 0 THEN
        RETURN 0.0
    END IF

    RETURN dot / (norm_x * norm_y)
END

SUBROUTINE: TopKIndices
INPUT: array[N], k
OUTPUT: indices[k]
BEGIN
    // Create (index, value) pairs
    pairs ← EMPTY_ARRAY[N]
    FOR i ← 0 TO N-1 DO
        pairs[i] ← (i, array[i])
    END FOR

    // Sort by value (descending)
    Sort(pairs, by=value, order=descending)

    // Extract top-k indices
    indices ← EMPTY_ARRAY[k]
    FOR i ← 0 TO k-1 DO
        indices[i] ← pairs[i].index
    END FOR

    RETURN indices
END
```

---

## 5. Adaptive Attention

### 5.1 Mixture of Experts (MoE) Attention

**Purpose**: Route to specialized attention mechanisms based on context

**Complexity**:
- Time: O(K · attention_complexity) where K = top-k experts (typically 2)
- Space: O(num_experts · model_size)

```
ALGORITHM: MoEAttention
INPUT:
    query: query node embedding [d]
    keys: neighbor embeddings [n × d]
    values: neighbor embeddings [n × d]
    experts: list of attention mechanisms
    router: routing network
    top_k: number of experts to use (typically 2)
OUTPUT:
    output: expert-mixed output [d]

EXPERT_TYPES:
    1. Standard Multi-Head Attention
    2. Hyperbolic Attention
    3. Linear Attention
    4. Edge-Featured Attention

BEGIN
    num_experts ← Length(experts)

    // 1. Router computes expert scores
    router_logits ← RouterNetwork(query, router)
    router_probs ← Softmax(router_logits)

    // 2. Select top-k experts
    top_k_indices ← TopKIndices(router_probs, top_k)

    // 3. Normalize selected expert weights
    selected_weights ← EMPTY_ARRAY[top_k]
    weight_sum ← 0.0
    FOR i ← 0 TO top_k-1 DO
        expert_idx ← top_k_indices[i]
        selected_weights[i] ← router_probs[expert_idx]
        weight_sum ← weight_sum + selected_weights[i]
    END FOR

    // Normalize
    FOR i ← 0 TO top_k-1 DO
        selected_weights[i] ← selected_weights[i] / weight_sum
    END FOR

    // 4. Compute weighted expert outputs
    output ← ZeroVector(d)
    FOR i ← 0 TO top_k-1 DO
        expert_idx ← top_k_indices[i]
        expert ← experts[expert_idx]

        // Call appropriate expert
        expert_output ← CALL_EXPERT(expert, query, keys, values)

        // Weighted accumulation
        weight ← selected_weights[i]
        FOR j ← 0 TO d-1 DO
            output[j] ← output[j] + weight * expert_output[j]
        END FOR
    END FOR

    RETURN output
END

SUBROUTINE: RouterNetwork
INPUT: query[d], router_weights
OUTPUT: logits[num_experts]
BEGIN
    // Simple two-layer MLP
    hidden_size ← 4 * d

    // First layer
    W1 ← router_weights.layer1  // [hidden_size × d]
    b1 ← router_weights.bias1    // [hidden_size]
    hidden ← MatrixVectorMult(W1, query)
    FOR i ← 0 TO hidden_size-1 DO
        hidden[i] ← ReLU(hidden[i] + b1[i])
    END FOR

    // Second layer
    W2 ← router_weights.layer2  // [num_experts × hidden_size]
    b2 ← router_weights.bias2    // [num_experts]
    logits ← MatrixVectorMult(W2, hidden)
    FOR i ← 0 TO num_experts-1 DO
        logits[i] ← logits[i] + b2[i]
    END FOR

    RETURN logits
END

SUBROUTINE: CALL_EXPERT
INPUT: expert, query, keys, values
OUTPUT: expert_output[d]
BEGIN
    MATCH expert.type:
        CASE "standard":
            RETURN MultiHeadAttention(query, keys, values, num_heads=8)

        CASE "hyperbolic":
            RETURN HyperbolicAttention(query, keys, values, curvature=-1.0)

        CASE "linear":
            RETURN LinearAttention(query, keys, values, num_features=256)

        CASE "edge_featured":
            edge_features ← expert.edge_features
            RETURN EdgeFeaturedAttention(query, keys, values, edge_features)

        DEFAULT:
            ERROR "Unknown expert type"
    END MATCH
END

SUBROUTINE: ReLU
INPUT: x (scalar)
OUTPUT: max(0, x)
BEGIN
    RETURN IF (x > 0) THEN x ELSE 0
END
```

---

### 5.2 Learned Navigation (Reinforcement Learning)

**Purpose**: Learn optimal navigation policy for graph traversal

**Complexity**:
- Time: O(num_steps · d²) per navigation episode
- Space: O(graph_size + policy_params)

```
ALGORITHM: RLNavigationStep
INPUT:
    current_state: current navigation state
    policy_network: learned policy (neural network)
    value_network: value estimator
    graph: graph structure
OUTPUT:
    action: which neighbor to visit
    reward: immediate reward
    next_state: resulting state

STATE_REPRESENTATION:
    current_embedding: [d]
    query_embedding: [d]
    graph_features: [d_graph]
    history: [max_steps × d]

BEGIN
    // 1. Encode current state
    state_vector ← EncodeState(current_state)

    // 2. Policy network outputs action logits
    action_logits ← PolicyNetwork(state_vector, policy_network)

    // 3. Value network estimates state value
    state_value ← ValueNetwork(state_vector, value_network)

    // 4. Sample action from policy
    action_probs ← Softmax(action_logits)
    action ← SampleCategorical(action_probs)  // Which neighbor to visit

    // 5. Execute action (move to selected neighbor)
    next_node ← current_state.neighbors[action]

    // 6. Compute reward
    reward ← ComputeReward(current_state, next_node, current_state.query)

    // 7. Update state
    next_state ← UpdateState(current_state, next_node, action)

    RETURN action, reward, next_state, state_value
END

SUBROUTINE: EncodeState
INPUT: state
OUTPUT: state_vector[d_state]
BEGIN
    // Concatenate all state components
    state_vector ← Concatenate(
        state.current_embedding,
        state.query_embedding,
        state.graph_features,
        Flatten(state.history)
    )

    RETURN state_vector
END

SUBROUTINE: PolicyNetwork
INPUT: state_vector[d_state], policy_params
OUTPUT: action_logits[num_neighbors]
BEGIN
    // Three-layer MLP
    hidden1 ← ReLU(Linear(state_vector, policy_params.W1, policy_params.b1))
    hidden2 ← ReLU(Linear(hidden1, policy_params.W2, policy_params.b2))
    logits ← Linear(hidden2, policy_params.W3, policy_params.b3)

    RETURN logits
END

SUBROUTINE: ValueNetwork
INPUT: state_vector[d_state], value_params
OUTPUT: value (scalar)
BEGIN
    // Three-layer MLP ending in scalar
    hidden1 ← ReLU(Linear(state_vector, value_params.W1, value_params.b1))
    hidden2 ← ReLU(Linear(hidden1, value_params.W2, value_params.b2))
    value ← Linear(hidden2, value_params.W3, value_params.b3)[0]  // Scalar output

    RETURN value
END

SUBROUTINE: ComputeReward
INPUT: current_state, next_node, query
OUTPUT: reward (scalar)
BEGIN
    // Reward based on similarity improvement
    current_similarity ← CosineSimilarity(
        current_state.current_embedding,
        query
    )

    next_similarity ← CosineSimilarity(
        next_node.embedding,
        query
    )

    // Positive reward if moving closer, negative if farther
    reward ← next_similarity - current_similarity

    // Bonus for reaching goal
    IF next_similarity > GOAL_THRESHOLD THEN
        reward ← reward + GOAL_BONUS
    END IF

    // Penalty for taking too many steps
    reward ← reward - STEP_PENALTY

    RETURN reward
END

SUBROUTINE: SampleCategorical
INPUT: probabilities[n]
OUTPUT: sampled_index in [0, n-1]
BEGIN
    // Sample from categorical distribution
    cumsum ← 0.0
    rand ← Random()  // Uniform [0, 1)

    FOR i ← 0 TO n-1 DO
        cumsum ← cumsum + probabilities[i]
        IF rand < cumsum THEN
            RETURN i
        END IF
    END FOR

    // Fallback (shouldn't reach here if probabilities sum to 1)
    RETURN n-1
END

SUBROUTINE: UpdateState
INPUT: current_state, next_node, action
OUTPUT: new_state
BEGIN
    new_state ← COPY(current_state)

    // Update current node
    new_state.current_node ← next_node
    new_state.current_embedding ← next_node.embedding

    // Update history (sliding window)
    new_state.history.PopFirst()
    new_state.history.Append(next_node.embedding)

    // Increment step counter
    new_state.num_steps ← new_state.num_steps + 1

    RETURN new_state
END

SUBROUTINE: Linear
INPUT: x[d_in], W[d_out × d_in], b[d_out]
OUTPUT: y[d_out]
BEGIN
    y ← MatrixVectorMult(W, x)
    FOR i ← 0 TO d_out-1 DO
        y[i] ← y[i] + b[i]
    END FOR
    RETURN y
END
```

---

## 6. Training Procedures

### 6.1 InfoNCE Contrastive Loss

**Purpose**: Learn embeddings that are similar to positives and dissimilar to negatives

**Complexity**:
- Time: O((n_pos + n_neg) · d)
- Space: O(n_pos + n_neg)

```
ALGORITHM: InfoNCELoss
INPUT:
    anchor: anchor embedding [d]
    positives: positive samples [n_pos × d]
    negatives: negative samples [n_neg × d]
    temperature: softmax temperature (typically 0.07)
OUTPUT:
    loss: contrastive loss (scalar)

BEGIN
    // 1. Compute positive similarities
    pos_scores ← EMPTY_ARRAY[n_pos]
    FOR i ← 0 TO n_pos-1 DO
        sim ← CosineSimilarity(anchor, positives[i])
        pos_scores[i] ← sim / temperature
    END FOR

    // 2. Compute negative similarities
    neg_scores ← EMPTY_ARRAY[n_neg]
    FOR i ← 0 TO n_neg-1 DO
        sim ← CosineSimilarity(anchor, negatives[i])
        neg_scores[i] ← sim / temperature
    END FOR

    // 3. InfoNCE loss (average over positives)
    total_loss ← 0.0

    FOR i ← 0 TO n_pos-1 DO
        // Numerator: exp(positive score)
        numerator ← exp(pos_scores[i])

        // Denominator: sum of exp(positive score) + all exp(negative scores)
        denominator ← numerator
        FOR j ← 0 TO n_neg-1 DO
            denominator ← denominator + exp(neg_scores[j])
        END FOR

        // Log probability
        log_prob ← log(numerator / denominator)

        // Accumulate negative log probability
        total_loss ← total_loss - log_prob
    END FOR

    // Average over positives
    loss ← total_loss / n_pos

    RETURN loss
END
```

---

### 6.2 Hard Negative Sampling

**Purpose**: Select informative negative samples for faster learning

**Complexity**:
- Time: O(N·d) where N = total number of samples
- Space: O(k) where k = number of hard negatives

```
ALGORITHM: SampleHardNegatives
INPUT:
    anchor: anchor embedding [d]
    all_embeddings: all available embeddings [N × d]
    true_positives: indices of true positives
    k: number of hard negatives to sample
    strategy: sampling strategy ("distance", "degree", "mixed")
OUTPUT:
    hard_negatives: selected hard negative samples [k × d]

BEGIN
    // 1. Filter out true positives
    candidate_indices ← EMPTY_LIST
    FOR i ← 0 TO N-1 DO
        IF i NOT IN true_positives THEN
            candidate_indices.Append(i)
        END IF
    END FOR

    n_candidates ← Length(candidate_indices)

    // 2. Select hard negatives based on strategy
    MATCH strategy:
        CASE "distance":
            hard_negatives ← SampleByDistance(
                anchor, all_embeddings, candidate_indices, k
            )

        CASE "degree":
            hard_negatives ← SampleByDegree(
                anchor, all_embeddings, candidate_indices, k
            )

        CASE "mixed":
            k_dist ← k / 2
            k_deg ← k - k_dist

            dist_negs ← SampleByDistance(
                anchor, all_embeddings, candidate_indices, k_dist
            )
            deg_negs ← SampleByDegree(
                anchor, all_embeddings, candidate_indices, k_deg
            )

            hard_negatives ← Concatenate(dist_negs, deg_negs)

        DEFAULT:
            ERROR "Unknown strategy"
    END MATCH

    RETURN hard_negatives
END

SUBROUTINE: SampleByDistance
INPUT: anchor[d], all_embeddings[N × d], candidate_indices, k
OUTPUT: hard_negatives[k × d]
BEGIN
    // Select k most similar candidates (hardest negatives)
    similarities ← EMPTY_ARRAY[Length(candidate_indices)]

    FOR i ← 0 TO Length(candidate_indices)-1 DO
        idx ← candidate_indices[i]
        similarities[i] ← CosineSimilarity(anchor, all_embeddings[idx])
    END FOR

    // Get top-k most similar (hardest)
    top_k_local_indices ← TopKIndices(similarities, k)

    // Map back to global indices
    hard_negatives ← EMPTY_MATRIX[k × d]
    FOR i ← 0 TO k-1 DO
        local_idx ← top_k_local_indices[i]
        global_idx ← candidate_indices[local_idx]
        hard_negatives[i] ← all_embeddings[global_idx]
    END FOR

    RETURN hard_negatives
END

SUBROUTINE: SampleByDegree
INPUT: anchor[d], all_embeddings[N × d], candidate_indices, k
OUTPUT: hard_negatives[k × d]
BEGIN
    // Select candidates with similar degree to anchor
    anchor_degree ← GetDegree(anchor)

    degree_diffs ← EMPTY_ARRAY[Length(candidate_indices)]
    FOR i ← 0 TO Length(candidate_indices)-1 DO
        idx ← candidate_indices[i]
        candidate_degree ← GetDegree(all_embeddings[idx])
        degree_diffs[i] ← abs(anchor_degree - candidate_degree)
    END FOR

    // Get k candidates with most similar degree
    top_k_local_indices ← TopKIndices(
        NegateArray(degree_diffs),  // Negate for similarity
        k
    )

    hard_negatives ← EMPTY_MATRIX[k × d]
    FOR i ← 0 TO k-1 DO
        local_idx ← top_k_local_indices[i]
        global_idx ← candidate_indices[local_idx]
        hard_negatives[i] ← all_embeddings[global_idx]
    END FOR

    RETURN hard_negatives
END

SUBROUTINE: NegateArray
INPUT: array[n]
OUTPUT: negated[n]
BEGIN
    negated ← EMPTY_ARRAY[n]
    FOR i ← 0 TO n-1 DO
        negated[i] ← -array[i]
    END FOR
    RETURN negated
END
```

---

### 6.3 Curriculum Learning Schedule

**Purpose**: Gradually increase task difficulty during training

**Complexity**:
- Time: O(1) per epoch (just weight computation)
- Space: O(num_losses)

```
ALGORITHM: CurriculumSchedule
INPUT:
    current_epoch: current training epoch
    total_epochs: total number of epochs
    loss_types: list of loss components
OUTPUT:
    loss_weights: weight for each loss component

LOSS_TYPES:
    - reconstruction: Autoencoder reconstruction loss
    - contrastive: InfoNCE contrastive loss
    - task: Downstream task loss
    - spectral: Laplacian regularization
    - ewc: Elastic Weight Consolidation

BEGIN
    loss_weights ← EMPTY_MAP

    // 1. Reconstruction: High early, decay exponentially
    lambda_recon ← exp(-current_epoch / 50.0)
    loss_weights["reconstruction"] ← lambda_recon

    // 2. Contrastive: Ramp up linearly in first 10 epochs
    IF current_epoch < 10 THEN
        lambda_contrast ← 0.1 + 0.9 * (current_epoch / 10.0)
    ELSE
        lambda_contrast ← 1.0
    END IF
    loss_weights["contrastive"] ← lambda_contrast

    // 3. Task: Start after 50 epochs, ramp up
    IF current_epoch < 50 THEN
        lambda_task ← 0.1
    ELSE
        lambda_task ← 0.1 + 0.9 * ((current_epoch - 50) / 50.0)
        lambda_task ← Min(lambda_task, 1.0)
    END IF
    loss_weights["task"] ← lambda_task

    // 4. Spectral: Constant moderate weight
    loss_weights["spectral"] ← 0.01

    // 5. EWC: Increase if using continual learning
    IF using_continual_learning THEN
        lambda_ewc ← Min(current_epoch / 100.0, 1.0)
    ELSE
        lambda_ewc ← 0.0
    END IF
    loss_weights["ewc"] ← lambda_ewc

    RETURN loss_weights
END
```

---

### 6.4 Multi-Objective Loss Computation

**Purpose**: Combine multiple loss functions with learned or scheduled weights

**Complexity**:
- Time: O(num_losses)
- Space: O(1)

```
ALGORITHM: MultiObjectiveLoss
INPUT:
    loss_components: computed loss values
    loss_weights: weights for each component
    auto_balance: whether to auto-balance weights
OUTPUT:
    total_loss: weighted sum of losses
    updated_weights: potentially updated weights

LOSS_COMPONENTS:
    task_loss: Main task objective
    contrastive_loss: InfoNCE or similar
    reconstruction_loss: Autoencoder
    spectral_loss: Laplacian smoothness
    ewc_loss: Continual learning penalty

BEGIN
    // 1. Auto-balance (optional)
    IF auto_balance THEN
        loss_weights ← AutoBalance(loss_components, loss_weights)
    END IF

    // 2. Compute weighted sum
    total_loss ← 0.0

    total_loss ← total_loss + loss_weights["task"] * loss_components.task_loss
    total_loss ← total_loss + loss_weights["contrastive"] * loss_components.contrastive_loss
    total_loss ← total_loss + loss_weights["reconstruction"] * loss_components.reconstruction_loss
    total_loss ← total_loss + loss_weights["spectral"] * loss_components.spectral_loss
    total_loss ← total_loss + loss_weights["ewc"] * loss_components.ewc_loss

    RETURN total_loss, loss_weights
END

SUBROUTINE: AutoBalance
INPUT: loss_components, current_weights
OUTPUT: balanced_weights
BEGIN
    // Normalize so each loss contributes equally
    num_losses ← 5

    // Compute current contribution of each loss
    contributions ← EMPTY_MAP
    contributions["task"] ← current_weights["task"] * loss_components.task_loss
    contributions["contrastive"] ← current_weights["contrastive"] * loss_components.contrastive_loss
    contributions["reconstruction"] ← current_weights["reconstruction"] * loss_components.reconstruction_loss
    contributions["spectral"] ← current_weights["spectral"] * loss_components.spectral_loss
    contributions["ewc"] ← current_weights["ewc"] * loss_components.ewc_loss

    // Compute total and target per-loss contribution
    total ← Sum(contributions.values)
    target_contribution ← total / num_losses

    // Adjust weights to equalize contributions
    balanced_weights ← EMPTY_MAP
    epsilon ← 1e-10  // Avoid division by zero

    balanced_weights["task"] ← target_contribution / Max(loss_components.task_loss, epsilon)
    balanced_weights["contrastive"] ← target_contribution / Max(loss_components.contrastive_loss, epsilon)
    balanced_weights["reconstruction"] ← target_contribution / Max(loss_components.reconstruction_loss, epsilon)
    balanced_weights["spectral"] ← target_contribution / Max(loss_components.spectral_loss, epsilon)
    balanced_weights["ewc"] ← target_contribution / Max(loss_components.ewc_loss, epsilon)

    RETURN balanced_weights
END
```

---

### 6.5 Spectral Regularization

**Purpose**: Preserve graph structure through Laplacian smoothness

**Complexity**:
- Time: O(|E|·d) where |E| = number of edges
- Space: O(1) (streaming computation)

```
ALGORITHM: LaplacianRegularization
INPUT:
    embeddings: node embeddings [N × d]
    edges: edge list [(u, v)]
    edge_weights: optional edge weights [|E|]
    normalized: whether to use normalized Laplacian
    node_degrees: node degrees [N]
OUTPUT:
    spectral_loss: smoothness penalty (scalar)

BEGIN
    total_loss ← 0.0
    num_edges ← Length(edges)

    FOR i ← 0 TO num_edges-1 DO
        u, v ← edges[i]

        // Compute embedding difference
        diff ← Subtract(embeddings[u], embeddings[v])
        diff_norm_sq ← L2NormSquared(diff)

        // Get edge weight
        weight ← 1.0
        IF edge_weights PROVIDED THEN
            weight ← edge_weights[i]
        END IF

        // Normalized Laplacian: weight by degrees
        IF normalized THEN
            degree_norm ← sqrt(node_degrees[u] * node_degrees[v])
            weight ← weight / Max(degree_norm, 1.0)
        END IF

        // Accumulate weighted squared difference
        total_loss ← total_loss + weight * diff_norm_sq
    END FOR

    // Average over edges
    spectral_loss ← total_loss / num_edges

    RETURN spectral_loss
END
```

---

## 7. Data Structures

### 7.1 Attention State

```
STRUCTURE: AttentionState
FIELDS:
    query: [d]                          // Query embedding
    keys: [n × d]                       // Key embeddings
    values: [n × d]                     // Value embeddings
    attention_weights: [n]              // Computed weights
    output: [d]                         // Final output
    metadata: Map<String, Any>          // Additional info

OPERATIONS:
    Initialize(query, keys, values)
    ComputeWeights() → attention_weights
    ComputeOutput() → output
    GetMetadata(key) → value
```

---

### 7.2 Graph Structure

```
STRUCTURE: Graph
FIELDS:
    nodes: [N]                          // Node identifiers
    embeddings: [N × d]                 // Node embeddings
    adjacency: [N × N] OR SparseMatrix  // Adjacency matrix
    edge_list: [(u, v)]                 // Edge list
    edge_features: [|E| × d_edge]       // Edge attributes
    node_degrees: [N]                   // Degree of each node

OPERATIONS:
    GetNeighbors(node_id) → [neighbor_ids]
    GetEdgeFeature(u, v) → [d_edge]
    GetDegree(node_id) → scalar
    AddEdge(u, v, features)
    UpdateEmbedding(node_id, new_embedding)
```

---

### 7.3 HNSW-Specific Structure

```
STRUCTURE: HNSWGraph
EXTENDS: Graph
ADDITIONAL_FIELDS:
    layers: [max_layer]                 // Layer-wise graphs
    entry_point: node_id                // Top-layer entry
    max_layer: integer                  // Maximum layer
    layer_neighbors: Map<(node, layer), [neighbors]>

OPERATIONS:
    GetLayerNeighbors(node_id, layer) → [neighbor_ids]
    GetNodeLayer(node_id) → layer
    NavigateLayer(query, layer, num_steps) → closest_node
    InsertNode(node_id, embedding, layer)
```

---

### 7.4 Training State

```
STRUCTURE: TrainingState
FIELDS:
    current_epoch: integer
    loss_history: [num_epochs]
    loss_weights: Map<loss_type, weight>
    curriculum_schedule: CurriculumSchedule
    optimizer_state: OptimizerState
    best_model_params: ModelParams
    early_stopping_counter: integer

OPERATIONS:
    UpdateEpoch()
    RecordLoss(loss_value)
    GetLossWeight(loss_type) → weight
    UpdateBestModel(current_params)
    ShouldEarlystop() → boolean
```

---

## 8. Complexity Summary

### 8.1 Attention Mechanisms

| Mechanism | Time Complexity | Space Complexity | Notes |
|-----------|----------------|------------------|-------|
| Scaled Dot-Product | O(n·d²) | O(n) | Standard attention |
| Multi-Head (h heads) | O(n·d²/h) | O(h·d) | Parallel heads |
| Hyperbolic | O(n·d²) | O(n) | More expensive ops |
| Sparse (Local+Global) | O((k_l + k_g)·d) | O(k_l + k_g) | k << n |
| Linear (Performer) | O(n·D·d) | O(D·d) | D = random features |
| Flash | O(n²·d) | O(n) | Better cache locality |
| Edge-Featured | O(n·(d² + d_edge·d)) | O(n) | Added edge cost |
| RoPE | O(n·d²) | O(n) | Rotation overhead minimal |
| Cross-Space | O(n_g·d² + k_l·d²) | O(n_g + k_l) | Dual attention |
| MoE (k experts) | O(k·base_complexity) | O(num_experts·model_size) | Expert routing |

**Legend**:
- n: number of neighbors/keys
- d: embedding dimension
- h: number of attention heads
- k_l, k_g: local and global neighbor counts
- D: number of random features
- d_edge: edge feature dimension

---

### 8.2 Training Operations

| Operation | Time Complexity | Space Complexity | Notes |
|-----------|----------------|------------------|-------|
| InfoNCE Loss | O((n_pos + n_neg)·d) | O(n_pos + n_neg) | Per anchor |
| Hard Negative Sampling | O(N·d) | O(k) | N = total samples |
| Spectral Regularization | O(\|E\|·d) | O(1) | E = edges |
| Curriculum Schedule | O(1) | O(num_losses) | Per epoch |
| Multi-Objective Loss | O(num_losses) | O(1) | Weighted sum |

---

## 9. Implementation Notes

### 9.1 Numerical Stability

**Softmax Stability**:
```
// Always subtract max before exp
max_score ← Max(scores)
exp_scores[i] ← exp(scores[i] - max_score)
```

**Hyperbolic Boundary**:
```
// Ensure points stay in Poincaré ball
IF ||x|| >= 1.0 THEN
    x ← 0.99 * x / ||x||  // Project back with margin
END IF
```

**Division by Zero**:
```
// Add epsilon to denominators
result ← numerator / (denominator + 1e-10)
```

---

### 9.2 Performance Optimization

**Vectorization**:
- Use SIMD operations for dot products
- Batch matrix multiplications
- Parallelize independent attention heads

**Memory Layout**:
- Contiguous memory for cache efficiency
- Column-major for matrix operations
- Pre-allocate buffers

**Lazy Computation**:
- Only compute attention weights when needed
- Cache frequently accessed embeddings
- Prune low-weight attention connections

---

### 9.3 Testing Strategies

**Unit Tests**:
```
TEST: ScaledDotProductAttention
    INPUT: Known query, keys, values
    EXPECTED: Hand-computed output
    VERIFY: Output matches expected within tolerance

TEST: Softmax Numerical Stability
    INPUT: Very large scores [1000, 999, 998]
    VERIFY: No NaN or Inf in output
    VERIFY: Probabilities sum to 1.0

TEST: Hyperbolic Boundary
    INPUT: Points near ball boundary (||x|| = 0.99)
    VERIFY: Result still in ball (||result|| < 1.0)
```

**Integration Tests**:
```
TEST: End-to-End Attention Pipeline
    INPUT: Real graph structure
    VERIFY: All mechanisms produce valid outputs
    VERIFY: Outputs are differentiable
```

**Performance Tests**:
```
BENCHMARK: Attention Complexity
    INPUT: Varying n = [10, 100, 1000, 10000]
    MEASURE: Time and memory usage
    VERIFY: Matches theoretical complexity
```

---

## 10. References

### 10.1 Core Papers

1. **Attention Mechanism**: Vaswani et al. (2017) - "Attention Is All You Need"
2. **GAT**: Veličković et al. (2018) - "Graph Attention Networks"
3. **Hyperbolic GNNs**: Chami et al. (2019) - "Hyperbolic Graph Convolutional Neural Networks"
4. **Performer**: Choromanski et al. (2020) - "Rethinking Attention with Performers"
5. **Flash Attention**: Dao et al. (2022) - "FlashAttention: Fast and Memory-Efficient Exact Attention"
6. **RoPE**: Su et al. (2021) - "RoFormer: Enhanced Transformer with Rotary Position Embedding"
7. **MoE**: Shazeer et al. (2017) - "Outrageously Large Neural Networks"

### 10.2 Mathematical Background

- **Hyperbolic Geometry**: Cannon et al. (1997) - "Hyperbolic Geometry"
- **Graph Laplacian**: Chung (1997) - "Spectral Graph Theory"
- **Contrastive Learning**: Chen et al. (2020) - "A Simple Framework for Contrastive Learning"

---

## 11. Glossary

**Attention**: Mechanism to weight importance of different inputs
**Multi-Head**: Parallel attention with different learned projections
**Hyperbolic Space**: Non-Euclidean geometry with constant negative curvature
**Poincaré Ball**: Conformal model of hyperbolic space in unit ball
**Möbius Addition**: Hyperbolic vector addition operation
**Sparse Attention**: Attention over subset of inputs (not all pairs)
**Linear Attention**: O(n) complexity via kernel approximation
**Flash Attention**: Memory-efficient tiled attention computation
**RoPE**: Rotary Position Embedding for distance encoding
**Cross-Attention**: Attention between two different spaces
**MoE**: Mixture of Experts, routing to specialized sub-models
**InfoNCE**: Noise Contrastive Estimation loss for contrastive learning
**Hard Negatives**: Difficult negative samples close to positives
**Curriculum Learning**: Gradually increasing task difficulty
**Spectral Regularization**: Graph smoothness via Laplacian

---

**Document Version**: 1.0
**Last Updated**: 2025-11-30
**Author**: RuVector Research Team
**SPARC Phase**: Pseudocode (Phase 2)
**Next Phase**: Architecture (Phase 3) - See `04-architecture.md`

---

## Appendix A: Quick Reference

### Common Subroutines

```
DotProduct(x, y) → scalar
L2Norm(x) → scalar
L2NormSquared(x) → scalar
Softmax(scores) → probabilities
CosineSimilarity(x, y) → similarity ∈ [-1, 1]
Scale(x, scalar) → scaled_vector
Add(x, y) → sum_vector
Subtract(x, y) → diff_vector
Concatenate(vectors...) → concatenated_vector
ZeroVector(d) → zero-initialized vector
ZeroMatrix(rows, cols) → zero-initialized matrix
```

### Complexity Quick Reference

```
O(1)     - Constant time
O(d)     - Linear in dimension
O(n)     - Linear in number of items
O(n·d)   - Linear in both
O(n²)    - Quadratic (standard full attention)
O(n·d²)  - Attention complexity
O(|E|)   - Linear in number of edges
```

---
