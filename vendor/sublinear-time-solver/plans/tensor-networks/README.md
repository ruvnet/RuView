# Tensor Network Methods for Exponentially Compressed Linear Solving

## Executive Summary

Tensor networks provide exponential compression of high-dimensional data by exploiting low-rank structure and entanglement patterns. For linear systems arising from discretized PDEs, quantum many-body problems, or machine learning, tensor networks can reduce complexity from O(2^n) to O(n·poly(r)) where r is the bond dimension. This enables solving previously intractable systems with billions of variables.

## Core Innovation: Exploiting Entanglement Structure

Real-world linear systems have structure:
1. **Local interactions** → Low entanglement → Small bond dimension
2. **Hierarchical correlations** → Tree tensor networks
3. **Translation symmetry** → Matrix Product States (MPS)
4. **Area law scaling** → Efficient tensor decomposition
5. **Exponential compression** → 10^9 parameters → 10^6 storage

## Tensor Network Architectures

### 1. Matrix Product States (MPS) / Tensor Trains (TT)

```python
class MPSSolver:
    """
    Solve Ax=b where A and b are in MPS/TT format
    """
    def __init__(self, max_bond_dim=100, tolerance=1e-6):
        self.max_bond = max_bond_dim
        self.tolerance = tolerance

    def solve_in_tt_format(self, A_tt, b_tt):
        """
        Never form full tensor - stay in compressed format!
        """
        # A is Matrix Product Operator (MPO)
        # b is Matrix Product State (MPS)
        # Solution x will be MPS

        # Initialize random MPS for solution
        x_tt = self.random_mps(b_tt.shape, bond_dim=10)

        # DMRG-style sweeping optimization
        for sweep in range(self.max_sweeps):
            # Right-to-left sweep
            for site in range(len(x_tt) - 1, 0, -1):
                x_tt = self.optimize_site(A_tt, b_tt, x_tt, site, direction='left')

            # Left-to-right sweep
            for site in range(len(x_tt) - 1):
                x_tt = self.optimize_site(A_tt, b_tt, x_tt, site, direction='right')

            # Check convergence
            residual = self.tt_residual_norm(A_tt, x_tt, b_tt)
            if residual < self.tolerance:
                break

            # Adaptive bond dimension
            if sweep % 5 == 0:
                x_tt = self.increase_bond_dimension(x_tt)

        return x_tt

    def optimize_site(self, A_mpo, b_mps, x_mps, site, direction='right'):
        """
        Local optimization of one tensor in the MPS
        """
        # Build effective Hamiltonian for this site
        H_eff = self.build_effective_hamiltonian(A_mpo, x_mps, site)

        # Local problem: H_eff * x_local = b_local
        b_local = self.extract_local_vector(b_mps, site)

        # Solve small local problem
        x_local = np.linalg.solve(H_eff, b_local)

        # Decompose and update MPS
        if direction == 'right':
            # QR decomposition for right-canonical form
            x_local_reshaped = x_local.reshape(
                x_mps[site].shape[0], -1
            )
            Q, R = np.linalg.qr(x_local_reshaped)

            # Update current site
            x_mps[site] = Q.reshape(x_mps[site].shape[0], -1, Q.shape[1])

            # Pass R to next site
            if site < len(x_mps) - 1:
                x_mps[site + 1] = np.tensordot(R, x_mps[site + 1], axes=(1, 0))
        else:
            # LQ decomposition for left-canonical form
            x_local_reshaped = x_local.reshape(
                -1, x_mps[site].shape[-1]
            )
            L, Q = np.linalg.qr(x_local_reshaped.T)

            # Update current site
            x_mps[site] = Q.T.reshape(L.T.shape[0], -1, x_mps[site].shape[-1])

            # Pass L to previous site
            if site > 0:
                x_mps[site - 1] = np.tensordot(x_mps[site - 1], L.T, axes=(-1, 0))

        # Truncate bond dimension
        x_mps = self.truncate_bond(x_mps, site)

        return x_mps

    def tt_matrix_vector_product(self, A_mpo, x_mps):
        """
        Compute Ax in tensor train format
        Complexity: O(n r³) instead of O(n²)
        """
        result = []

        for i in range(len(x_mps)):
            # Contract MPO tensor with MPS tensor at each site
            contracted = np.tensordot(A_mpo[i], x_mps[i], axes=([2], [1]))

            # Reshape for next operation
            result.append(contracted.transpose(0, 2, 1, 3).reshape(
                contracted.shape[0] * contracted.shape[2],
                contracted.shape[1],
                contracted.shape[3]
            ))

        return result
```

### 2. Projected Entangled Pair States (PEPS)

```python
class PEPSSolver:
    """
    2D tensor network for solving grid/lattice problems
    """
    def __init__(self, grid_shape, bond_dim=10):
        self.shape = grid_shape
        self.bond_dim = bond_dim

    def solve_2d_system(self, A_peps, b_peps):
        """
        Solve where A is 2D tensor network operator
        """
        # Initialize solution as PEPS
        x_peps = self.random_peps(self.shape, self.bond_dim)

        # Imaginary time evolution
        beta = 0.01  # Inverse temperature

        for step in range(self.max_steps):
            # Apply exp(-beta * A) to x
            x_peps = self.imaginary_time_evolution(A_peps, x_peps, beta)

            # Project onto constraint Ax = b
            x_peps = self.project_onto_constraint(x_peps, A_peps, b_peps)

            # Increase beta (cool down)
            beta *= 1.1

            # Check convergence
            if self.check_convergence(x_peps, A_peps, b_peps):
                break

        return x_peps

    def contract_peps_network(self, peps):
        """
        Contract 2D tensor network (NP-hard in general!)
        Use boundary MPS method
        """
        height, width = peps.shape

        # Start from top row as MPS
        boundary_mps = peps[0, :]

        # Absorb rows one by one
        for row in range(1, height):
            # Current row as MPS
            current_row = peps[row, :]

            # Contract boundary MPS with current row
            boundary_mps = self.contract_mps_with_mps(
                boundary_mps,
                current_row,
                max_bond=self.bond_dim * 2
            )

            # Compress to maintain bond dimension
            boundary_mps = self.compress_mps(boundary_mps, self.bond_dim)

        # Final contraction gives scalar
        return self.contract_mps_to_scalar(boundary_mps)
```

### 3. Tree Tensor Networks (TTN)

```rust
// Hierarchical tensor decomposition for structured problems
struct TreeTensorNetwork {
    root: TensorNode,
    levels: Vec<Vec<TensorNode>>,
    bond_dims: Vec<usize>,
}

impl TreeTensorNetwork {
    fn solve_hierarchical(&mut self, A: &TTNOperator, b: &TTNState) -> TTNState {
        // Binary tree structure matches problem hierarchy

        // Bottom-up pass: Coarse-graining
        for level in (0..self.levels.len()).rev() {
            self.coarse_grain_level(level);
        }

        // Solve at root (small problem)
        let root_solution = self.solve_root(A, b);

        // Top-down pass: Refinement
        let mut solution = TTNState::from_root(root_solution);
        for level in 0..self.levels.len() {
            solution = self.refine_level(solution, level, A, b);
        }

        solution
    }

    fn coarse_grain_level(&mut self, level: usize) {
        // Combine pairs of tensors via SVD
        for i in (0..self.levels[level].len()).step_by(2) {
            let left = &self.levels[level][i];
            let right = &self.levels[level][i + 1];

            // Contract tensors
            let combined = contract_tensors(left, right);

            // SVD to get parent tensor
            let (u, s, v) = svd_truncated(combined, self.bond_dims[level]);

            // Store parent at higher level
            if level > 0 {
                self.levels[level - 1][i / 2] = u;
            } else {
                self.root = u;
            }

            // Store isometry for later refinement
            self.levels[level][i].set_isometry(s * v.t());
        }
    }

    fn refine_level(
        &self,
        coarse_solution: TTNState,
        level: usize,
        A: &TTNOperator,
        b: &TTNState,
    ) -> TTNState {
        let mut refined = TTNState::new(self.levels[level].len());

        for (i, node) in self.levels[level].iter().enumerate() {
            // Get coarse solution for this branch
            let coarse_component = coarse_solution.get_branch(i);

            // Local refinement problem
            let local_A = A.extract_local(level, i);
            let local_b = b.extract_local(level, i);

            // Solve with coarse solution as initial guess
            let refined_component = self.refine_local(
                local_A,
                local_b,
                coarse_component,
                node.get_isometry(),
            );

            refined.set_component(i, refined_component);
        }

        refined
    }
}
```

### 4. Multi-scale Entanglement Renormalization Ansatz (MERA)

```python
class MERASolver:
    """
    Tensor network with causal structure for critical systems
    """
    def __init__(self, system_size, num_levels=None):
        self.size = system_size
        self.levels = num_levels or int(np.log2(system_size))
        self.tensors = self.initialize_mera()

    def initialize_mera(self):
        """
        Build MERA structure with disentanglers and isometries
        """
        mera = {
            'disentanglers': [],  # Remove local entanglement
            'isometries': [],     # Coarse-grain
        }

        size = self.size
        for level in range(self.levels):
            # Disentanglers at this scale
            num_disentanglers = size // 2
            disentanglers = [
                np.random.randn(4, 4).reshape(2, 2, 2, 2)
                for _ in range(num_disentanglers)
            ]
            mera['disentanglers'].append(disentanglers)

            # Isometries for coarse-graining
            num_isometries = size // 2
            isometries = [
                np.random.randn(2, 4).reshape(2, 2, 2)
                for _ in range(num_isometries)
            ]
            mera['isometries'].append(isometries)

            size //= 2

        return mera

    def solve_critical_system(self, H, target_state):
        """
        Solve at quantum critical point where entanglement is maximal
        """
        # Optimize MERA tensors to represent ground state
        for iteration in range(self.max_iterations):
            # Ascending pass: Apply layers from bottom to top
            state = target_state
            environments = []

            for level in range(self.levels):
                # Apply disentanglers
                state = self.apply_disentanglers(state, level)

                # Apply isometries
                state = self.apply_isometries(state, level)

                # Store environment for backwards pass
                environments.append(self.compute_environment(H, state))

            # Descending pass: Update tensors
            for level in range(self.levels - 1, -1, -1):
                # Update isometries
                self.update_isometries(level, environments[level])

                # Update disentanglers
                self.update_disentanglers(level, environments[level])

            # Check energy convergence
            energy = self.compute_energy(H)
            if iteration > 0 and abs(energy - prev_energy) < 1e-10:
                break
            prev_energy = energy

        # Use optimized MERA to solve linear system
        return self.extract_solution()

    def apply_disentanglers(self, state, level):
        """
        Remove short-range entanglement
        """
        disentangled = state.copy()
        for i, disentangler in enumerate(self.tensors['disentanglers'][level]):
            # Apply to pairs of sites
            site1, site2 = 2*i, 2*i + 1
            local_state = state[site1:site2+1]

            # Contract with disentangler
            new_local = np.tensordot(disentangler, local_state, axes=([2, 3], [0, 1]))
            disentangled[site1:site2+1] = new_local

        return disentangled
```

## Advanced Algorithms

### 1. Tensor Cross Interpolation (TCI)

```python
class TensorCrossInterpolation:
    """
    Build tensor network by sampling O(nr²) elements
    instead of all n² elements!
    """
    def __init__(self):
        self.pivots = []
        self.factors = []

    def build_from_black_box(self, matrix_oracle, shape, max_rank=50):
        """
        matrix_oracle(i, j) returns A[i,j]
        Build TT approximation without seeing full matrix!
        """
        n = shape[0]
        d = int(np.log2(n))  # Assume n = 2^d

        # Initial random pivot
        pivot = [np.random.randint(2) for _ in range(d)]
        self.pivots = [pivot]

        # Build tensor train core by core
        tt_cores = []

        for k in range(d):
            # Select pivot rows and columns
            left_indices = self.select_indices(k, 'left')
            right_indices = self.select_indices(k, 'right')

            # Sample submatrix
            submatrix = np.zeros((len(left_indices), 2, len(right_indices)))
            for i, left_idx in enumerate(left_indices):
                for bit in range(2):
                    for j, right_idx in enumerate(right_indices):
                        # Query oracle
                        full_idx = self.combine_indices(left_idx, bit, right_idx, k)
                        submatrix[i, bit, j] = matrix_oracle(*full_idx)

            # Find optimal rank-r approximation
            core = self.find_optimal_core(submatrix, max_rank)
            tt_cores.append(core)

            # Update pivots using maximum volume principle
            self.update_pivots(core)

        return TTMatrix(tt_cores)

    def solve_via_cross(self, matrix_oracle, b, shape):
        """
        Solve Ax=b accessing only O(nr²) matrix elements
        """
        # Build TT approximation of A
        A_tt = self.build_from_black_box(matrix_oracle, shape)

        # Convert b to TT format
        b_tt = self.vector_to_tt(b)

        # Solve in TT format
        solver = MPSSolver()
        x_tt = solver.solve_in_tt_format(A_tt, b_tt)

        # Convert back to full vector
        return self.tt_to_vector(x_tt)
```

### 2. Tangent Space Methods

```python
class TangentSpaceSolver:
    """
    Optimize directly on manifold of fixed-rank tensors
    """
    def __init__(self, rank):
        self.rank = rank

    def solve_on_manifold(self, A, b):
        """
        Stay on low-rank manifold throughout optimization
        """
        # Initialize on manifold
        x = self.random_point_on_manifold(len(b), self.rank)

        # Riemannian conjugate gradient
        r = b - A @ x
        p = self.project_to_tangent(r, x)

        for iteration in range(self.max_iterations):
            # Line search along geodesic
            Ap = A @ p
            alpha = np.dot(r, p) / np.dot(p, Ap)

            # Move along geodesic
            x = self.retraction(x, alpha * p)

            # Update residual
            r_new = r - alpha * Ap

            # Project to tangent space at new point
            r_tangent = self.project_to_tangent(r_new, x)

            # Conjugate direction (Riemannian)
            beta = self.riemannian_metric(r_tangent, r_tangent, x) / \
                   self.riemannian_metric(p, p, x)

            p = r_tangent + beta * self.parallel_transport(p, x_old, x)

            r = r_new
            x_old = x

            if np.linalg.norm(r) < 1e-6:
                break

        return x

    def retraction(self, x, tangent_vector):
        """
        Map tangent vector to manifold
        """
        # QR-based retraction for fixed-rank manifold
        y = x + tangent_vector
        q, r = np.linalg.qr(y)
        return q @ r[:self.rank]

    def project_to_tangent(self, vector, point):
        """
        Project to tangent space of low-rank manifold
        """
        u, s, vt = np.linalg.svd(point, full_matrices=False)

        # Tangent space has specific structure
        tangent = u @ u.T @ vector @ vt.T @ vt + \
                 (np.eye(len(u)) - u @ u.T) @ vector @ vt.T @ vt + \
                 u @ u.T @ vector @ (np.eye(len(vt)) - vt.T @ vt)

        return tangent
```

### 3. Tensor Completion for Sparse Systems

```python
class TensorCompletionSolver:
    """
    Solve even when most matrix entries are unknown!
    """
    def __init__(self):
        self.observed_entries = {}

    def solve_from_samples(self, samples, b, shape):
        """
        samples: Dictionary of (i,j): A[i,j] for known entries
        Solve Ax=b knowing only ~O(n log n) entries of A!
        """
        # Nuclear norm minimization in TT format
        n = shape[0]

        # Initialize random TT
        X_tt = self.random_tt(shape, rank=10)

        # Alternating minimization
        for iteration in range(100):
            # Fix all cores except one, optimize that core
            for core_idx in range(len(X_tt.cores)):
                # Build linear system for this core
                A_local, b_local = self.build_local_system(
                    X_tt, core_idx, samples, b
                )

                # Solve for optimal core
                X_tt.cores[core_idx] = np.linalg.solve(A_local, b_local)

                # Orthogonalize for stability
                X_tt = self.orthogonalize_tt(X_tt, core_idx)

            # Check if we satisfy known entries
            error = self.compute_sampling_error(X_tt, samples)
            if error < 1e-10:
                break

            # Increase rank if needed
            if iteration % 10 == 0:
                X_tt = self.increase_rank(X_tt)

        return X_tt
```

## Performance Analysis

### Compression Ratios

| Problem Type | Full Storage | TT Storage | Compression |
|--------------|--------------|------------|-------------|
| 1D Chain (n=2^20) | 10^12 | 10^5 | 10^7× |
| 2D Grid (256×256) | 4×10^9 | 10^6 | 4000× |
| 3D Lattice (64³) | 7×10^10 | 10^7 | 7000× |
| Quantum Many-Body | 2^40 | 10^3 | 10^9× |

### Computational Complexity

```python
def complexity_comparison(n, rank):
    """
    Compare tensor network vs dense methods
    """
    dense = {
        'storage': n**2,
        'matvec': n**2,
        'solve': n**3,
    }

    tensor_network = {
        'storage': n * rank**2,
        'matvec': n * rank**3,
        'solve': n * rank**3 * log(n),  # Sweeps
    }

    speedup = {
        'storage': dense['storage'] / tensor_network['storage'],
        'matvec': dense['matvec'] / tensor_network['matvec'],
        'solve': dense['solve'] / tensor_network['solve'],
    }

    return speedup  # Often 1000-1000000×!
```

## Cutting-Edge Research

### Recent Breakthroughs

1. **Oseledets (2011)**: "Tensor-Train Decomposition"
   - Foundation of modern tensor methods
   - SIAM J. Sci. Comput.

2. **Schollwöck (2011)**: "The Density-Matrix Renormalization Group"
   - Comprehensive DMRG review
   - Annals of Physics

3. **Evenbly & Vidal (2014)**: "Tensor Network Renormalization"
   - TNR algorithm
   - Physical Review Letters

4. **Bridgeman & Chubb (2017)**: "Hand-waving and Interpretive Dance"
   - Intuitive tensor network guide
   - J. Phys. A

5. **Ran et al. (2020)**: "Tensor Network Contractions"
   - Optimization strategies
   - Lecture Notes in Physics

6. **Gray & Kourtis (2021)**: "Hyper-optimized Tensor Network Contraction"
   - quimb library
   - Quantum

### Software Libraries

- **ITensor** (C++/Julia): Production physics calculations
- **TensorNetwork** (Python): Google's TN library
- **quimb** (Python): Quantum information & many-body
- **TNQVM** (C++): Tensor network quantum VM
- **TeNPy** (Python): DMRG and more

## Applications to Sublinear Solving

```python
class SublinearTensorSolver:
    """
    Combine sublinear sampling with tensor compression
    """
    def __init__(self):
        self.tensor_format = 'TT'
        self.max_rank = 100

    def solve_sublinear_tensor(self, A_oracle, b, n):
        """
        A_oracle: Function that returns A[i,j]
        Never construct full matrix!
        """
        # Phase 1: Sketch the operator structure
        sketch_samples = self.importance_sampling(n, num_samples=100*self.max_rank)

        # Phase 2: Build tensor approximation from samples
        A_tn = TensorCrossInterpolation().build_from_samples(
            A_oracle, sketch_samples, shape=(n, n)
        )

        # Phase 3: Solve in tensor format
        b_tn = self.vector_to_tensor_network(b)
        x_tn = self.solve_in_tn_format(A_tn, b_tn)

        # Phase 4: Extract solution
        return self.tensor_network_to_vector(x_tn)

    def importance_sampling(self, n, num_samples):
        """
        Sample matrix entries based on leverage scores
        """
        samples = []

        # Estimate leverage scores via random projection
        k = int(np.log(n)) * 10
        random_matrix = np.random.randn(n, k) / np.sqrt(k)

        for _ in range(num_samples):
            # Sample row based on leverage
            i = self.sample_by_leverage(random_matrix)

            # Sample column uniformly (can be improved)
            j = np.random.randint(n)

            samples.append((i, j))

        return samples

    def solve_in_tn_format(self, A_tn, b_tn):
        """
        DMRG-style solver staying in tensor format
        """
        # Never expand to full matrix!
        solver = DMRGLinearSolver(max_bond=self.max_rank)
        return solver.solve(A_tn, b_tn)
```

## Conclusion

Tensor networks provide exponential compression for structured linear systems, reducing intractable problems to tractable ones. By combining with sublinear sampling, we can solve systems with billions of unknowns using only megabytes of memory. The key insight: real-world problems have low entanglement structure that tensor networks naturally exploit. This is the future of large-scale scientific computing.