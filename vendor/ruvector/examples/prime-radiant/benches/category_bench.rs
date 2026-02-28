//! Category Theory Benchmarks for Prime-Radiant
//!
//! Benchmarks for category-theoretic operations including:
//! - Functor application
//! - Morphism composition chains
//! - Topos operations (pullback, pushforward, exponential)
//! - Natural transformation computation
//!
//! Target metrics:
//! - Functor application: < 100us per object
//! - Composition chain (100 morphisms): < 1ms
//! - Topos pullback: < 500us

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;

// ============================================================================
// CATEGORY THEORY TYPES
// ============================================================================

/// Object identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ObjectId(u64);

/// Morphism identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MorphismId(u64);

/// A morphism in a category
#[derive(Clone, Debug)]
struct Morphism {
    id: MorphismId,
    source: ObjectId,
    target: ObjectId,
    /// Linear transformation matrix (for VectorCategory)
    matrix: Option<Vec<Vec<f64>>>,
}

/// Category structure
struct Category {
    objects: HashMap<ObjectId, Object>,
    morphisms: HashMap<MorphismId, Morphism>,
    /// Composition table: (f, g) -> f . g
    compositions: HashMap<(MorphismId, MorphismId), MorphismId>,
    /// Identity morphisms
    identities: HashMap<ObjectId, MorphismId>,
    next_id: u64,
}

/// Object with associated data
#[derive(Clone, Debug)]
struct Object {
    id: ObjectId,
    dimension: usize,
    data: Vec<f64>,
}

impl Category {
    fn new() -> Self {
        Self {
            objects: HashMap::new(),
            morphisms: HashMap::new(),
            compositions: HashMap::new(),
            identities: HashMap::new(),
            next_id: 0,
        }
    }

    fn add_object(&mut self, dimension: usize) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;

        let obj = Object {
            id,
            dimension,
            data: vec![0.0; dimension],
        };
        self.objects.insert(id, obj);

        // Add identity morphism
        let mor_id = MorphismId(self.next_id);
        self.next_id += 1;

        let identity_matrix = (0..dimension)
            .map(|i| {
                let mut row = vec![0.0; dimension];
                row[i] = 1.0;
                row
            })
            .collect();

        let identity = Morphism {
            id: mor_id,
            source: id,
            target: id,
            matrix: Some(identity_matrix),
        };

        self.morphisms.insert(mor_id, identity);
        self.identities.insert(id, mor_id);

        id
    }

    fn add_morphism(&mut self, source: ObjectId, target: ObjectId, matrix: Vec<Vec<f64>>) -> MorphismId {
        let id = MorphismId(self.next_id);
        self.next_id += 1;

        let morphism = Morphism {
            id,
            source,
            target,
            matrix: Some(matrix),
        };

        self.morphisms.insert(id, morphism);
        id
    }

    fn compose(&mut self, f: MorphismId, g: MorphismId) -> Option<MorphismId> {
        // Check if already composed
        if let Some(&result) = self.compositions.get(&(f, g)) {
            return Some(result);
        }

        let mor_f = self.morphisms.get(&f)?;
        let mor_g = self.morphisms.get(&g)?;

        // Check composability: target(g) = source(f)
        if mor_g.target != mor_f.source {
            return None;
        }

        // Compose matrices
        let mat_f = mor_f.matrix.as_ref()?;
        let mat_g = mor_g.matrix.as_ref()?;
        let composed_matrix = matrix_multiply(mat_f, mat_g);

        let new_id = self.add_morphism(mor_g.source, mor_f.target, composed_matrix);
        self.compositions.insert((f, g), new_id);

        Some(new_id)
    }

    fn compose_chain(&mut self, morphisms: &[MorphismId]) -> Option<MorphismId> {
        if morphisms.is_empty() {
            return None;
        }

        let mut result = morphisms[0];
        for &mor in morphisms.iter().skip(1) {
            result = self.compose(result, mor)?;
        }

        Some(result)
    }
}

/// Matrix multiplication
fn matrix_multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let m = a.len();
    let n = if b.is_empty() { 0 } else { b[0].len() };
    let k = b.len();

    let mut result = vec![vec![0.0; n]; m];

    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0;
            for l in 0..k {
                sum += a[i][l] * b[l][j];
            }
            result[i][j] = sum;
        }
    }

    result
}

// ============================================================================
// FUNCTOR IMPLEMENTATION
// ============================================================================

/// A functor between categories
struct Functor {
    /// Object mapping (encoded as transformation)
    object_map: Box<dyn Fn(&Object) -> Object + Send + Sync>,
    /// Morphism mapping
    morphism_map: Box<dyn Fn(&Morphism) -> Morphism + Send + Sync>,
}

impl Functor {
    /// Embedding functor: embeds into higher dimension
    fn embedding(target_dim: usize) -> Self {
        Self {
            object_map: Box::new(move |obj| {
                let mut data = obj.data.clone();
                data.resize(target_dim, 0.0);
                Object {
                    id: obj.id,
                    dimension: target_dim,
                    data,
                }
            }),
            morphism_map: Box::new(move |mor| {
                let matrix = mor.matrix.as_ref().map(|m| {
                    let old_dim = m.len();
                    let mut new_matrix = vec![vec![0.0; target_dim]; target_dim];

                    // Copy old matrix into top-left corner
                    for i in 0..old_dim {
                        for j in 0..m[i].len().min(target_dim) {
                            new_matrix[i][j] = m[i][j];
                        }
                    }

                    // Extend with identity
                    for i in old_dim..target_dim {
                        new_matrix[i][i] = 1.0;
                    }

                    new_matrix
                });

                Morphism {
                    id: mor.id,
                    source: mor.source,
                    target: mor.target,
                    matrix,
                }
            }),
        }
    }

    /// Projection functor: projects to lower dimension
    fn projection(target_dim: usize) -> Self {
        Self {
            object_map: Box::new(move |obj| {
                let data: Vec<f64> = obj.data.iter().take(target_dim).copied().collect();
                Object {
                    id: obj.id,
                    dimension: target_dim,
                    data,
                }
            }),
            morphism_map: Box::new(move |mor| {
                let matrix = mor.matrix.as_ref().map(|m| {
                    m.iter()
                        .take(target_dim)
                        .map(|row| row.iter().take(target_dim).copied().collect())
                        .collect()
                });

                Morphism {
                    id: mor.id,
                    source: mor.source,
                    target: mor.target,
                    matrix,
                }
            }),
        }
    }

    fn apply_object(&self, obj: &Object) -> Object {
        (self.object_map)(obj)
    }

    fn apply_morphism(&self, mor: &Morphism) -> Morphism {
        (self.morphism_map)(mor)
    }
}

// ============================================================================
// TOPOS OPERATIONS
// ============================================================================

/// Topos structure with subobject classifier
struct Topos {
    base_category: Category,
    /// Subobject classifier: true/false
    omega: ObjectId,
    /// Terminal object
    terminal: ObjectId,
}

impl Topos {
    fn new() -> Self {
        let mut cat = Category::new();

        // Add terminal object (1-dimensional)
        let terminal = cat.add_object(1);

        // Add subobject classifier (2-dimensional for true/false)
        let omega = cat.add_object(2);

        Self {
            base_category: cat,
            omega,
            terminal,
        }
    }

    /// Compute pullback of f: A -> C and g: B -> C
    fn pullback(&mut self, f: MorphismId, g: MorphismId) -> Option<(ObjectId, MorphismId, MorphismId)> {
        let mor_f = self.base_category.morphisms.get(&f)?;
        let mor_g = self.base_category.morphisms.get(&g)?;

        // Check that codomain matches
        if mor_f.target != mor_g.target {
            return None;
        }

        let obj_a = self.base_category.objects.get(&mor_f.source)?;
        let obj_b = self.base_category.objects.get(&mor_g.source)?;

        // Pullback object dimension is sum of source dimensions
        let pullback_dim = obj_a.dimension + obj_b.dimension;
        let pullback_obj = self.base_category.add_object(pullback_dim);

        // Create projection morphisms
        // p1: A x_C B -> A (projection to first factor)
        let p1_matrix: Vec<Vec<f64>> = (0..obj_a.dimension)
            .map(|i| {
                let mut row = vec![0.0; pullback_dim];
                row[i] = 1.0;
                row
            })
            .collect();

        // p2: A x_C B -> B (projection to second factor)
        let p2_matrix: Vec<Vec<f64>> = (0..obj_b.dimension)
            .map(|i| {
                let mut row = vec![0.0; pullback_dim];
                row[obj_a.dimension + i] = 1.0;
                row
            })
            .collect();

        let p1 = self.base_category.add_morphism(pullback_obj, mor_f.source, p1_matrix);
        let p2 = self.base_category.add_morphism(pullback_obj, mor_g.source, p2_matrix);

        Some((pullback_obj, p1, p2))
    }

    /// Compute exponential object B^A
    fn exponential(&mut self, a: ObjectId, b: ObjectId) -> Option<ObjectId> {
        let obj_a = self.base_category.objects.get(&a)?;
        let obj_b = self.base_category.objects.get(&b)?;

        // Exponential dimension is dim(B)^dim(A) (approximated as product)
        let exp_dim = obj_a.dimension * obj_b.dimension;
        let exp_obj = self.base_category.add_object(exp_dim);

        Some(exp_obj)
    }

    /// Compute pushout of f: C -> A and g: C -> B
    fn pushout(&mut self, f: MorphismId, g: MorphismId) -> Option<(ObjectId, MorphismId, MorphismId)> {
        let mor_f = self.base_category.morphisms.get(&f)?;
        let mor_g = self.base_category.morphisms.get(&g)?;

        // Check that domain matches
        if mor_f.source != mor_g.source {
            return None;
        }

        let obj_a = self.base_category.objects.get(&mor_f.target)?;
        let obj_b = self.base_category.objects.get(&mor_g.target)?;

        // Pushout dimension
        let pushout_dim = obj_a.dimension + obj_b.dimension;
        let pushout_obj = self.base_category.add_object(pushout_dim);

        // Create injection morphisms
        let i1_matrix: Vec<Vec<f64>> = (0..pushout_dim)
            .map(|i| {
                if i < obj_a.dimension {
                    let mut row = vec![0.0; obj_a.dimension];
                    row[i] = 1.0;
                    row
                } else {
                    vec![0.0; obj_a.dimension]
                }
            })
            .collect();

        let i2_matrix: Vec<Vec<f64>> = (0..pushout_dim)
            .map(|i| {
                if i >= obj_a.dimension {
                    let mut row = vec![0.0; obj_b.dimension];
                    row[i - obj_a.dimension] = 1.0;
                    row
                } else {
                    vec![0.0; obj_b.dimension]
                }
            })
            .collect();

        let i1 = self.base_category.add_morphism(mor_f.target, pushout_obj, i1_matrix);
        let i2 = self.base_category.add_morphism(mor_g.target, pushout_obj, i2_matrix);

        Some((pushout_obj, i1, i2))
    }
}

// ============================================================================
// NATURAL TRANSFORMATION
// ============================================================================

/// Natural transformation between functors
struct NaturalTransformation {
    /// Component morphisms for each object
    components: HashMap<ObjectId, Vec<Vec<f64>>>,
}

impl NaturalTransformation {
    fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    fn add_component(&mut self, obj: ObjectId, matrix: Vec<Vec<f64>>) {
        self.components.insert(obj, matrix);
    }

    fn apply_at(&self, obj: ObjectId, data: &[f64]) -> Option<Vec<f64>> {
        let matrix = self.components.get(&obj)?;
        Some(matvec(matrix, data))
    }

    /// Check naturality square for a morphism f: A -> B
    fn check_naturality(&self, f: &Morphism, f_prime: &Morphism) -> bool {
        // Check: F(f) . eta_A = eta_B . G(f)
        let eta_a = match self.components.get(&f.source) {
            Some(m) => m,
            None => return false,
        };
        let eta_b = match self.components.get(&f.target) {
            Some(m) => m,
            None => return false,
        };

        let mat_f = match &f.matrix {
            Some(m) => m,
            None => return false,
        };
        let mat_f_prime = match &f_prime.matrix {
            Some(m) => m,
            None => return false,
        };

        // Left side: F(f) . eta_A
        let left = matrix_multiply(mat_f_prime, eta_a);

        // Right side: eta_B . G(f)
        let right = matrix_multiply(eta_b, mat_f);

        // Check equality (within tolerance)
        matrices_equal(&left, &right, 1e-10)
    }
}

fn matvec(matrix: &[Vec<f64>], vec: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| row.iter().zip(vec.iter()).map(|(a, b)| a * b).sum())
        .collect()
}

fn matrices_equal(a: &[Vec<f64>], b: &[Vec<f64>], tol: f64) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for (row_a, row_b) in a.iter().zip(b.iter()) {
        if row_a.len() != row_b.len() {
            return false;
        }
        for (va, vb) in row_a.iter().zip(row_b.iter()) {
            if (va - vb).abs() > tol {
                return false;
            }
        }
    }

    true
}

// ============================================================================
// BENCHMARK DATA GENERATORS
// ============================================================================

fn generate_random_matrix(rows: usize, cols: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut rng_state = seed;

    (0..rows)
        .map(|_| {
            (0..cols)
                .map(|_| {
                    rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((rng_state >> 33) as f64 / (u32::MAX as f64)) * 2.0 - 1.0
                })
                .collect()
        })
        .collect()
}

fn setup_category_with_chain(dimension: usize, chain_length: usize) -> (Category, Vec<MorphismId>) {
    let mut cat = Category::new();
    let mut objects = Vec::new();
    let mut morphisms = Vec::new();

    // Create chain of objects
    for _ in 0..=chain_length {
        objects.push(cat.add_object(dimension));
    }

    // Create chain of morphisms
    for i in 0..chain_length {
        let matrix = generate_random_matrix(dimension, dimension, (i as u64) * 42 + 1);
        let mor = cat.add_morphism(objects[i], objects[i + 1], matrix);
        morphisms.push(mor);
    }

    (cat, morphisms)
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_functor_application(c: &mut Criterion) {
    let mut group = c.benchmark_group("category/functor");
    group.sample_size(100);

    for &dim in &[16, 64, 128, 256] {
        let target_dim = dim * 2;
        let embedding = Functor::embedding(target_dim);
        let projection = Functor::projection(dim / 2);

        let obj = Object {
            id: ObjectId(0),
            dimension: dim,
            data: (0..dim).map(|i| (i as f64).sin()).collect(),
        };

        let mor = Morphism {
            id: MorphismId(0),
            source: ObjectId(0),
            target: ObjectId(1),
            matrix: Some(generate_random_matrix(dim, dim, 42)),
        };

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(
            BenchmarkId::new("embedding_object", dim),
            &(&embedding, &obj),
            |b, (functor, obj)| {
                b.iter(|| black_box(functor.apply_object(black_box(obj))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("embedding_morphism", dim),
            &(&embedding, &mor),
            |b, (functor, mor)| {
                b.iter(|| black_box(functor.apply_morphism(black_box(mor))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("projection_object", dim),
            &(&projection, &obj),
            |b, (functor, obj)| {
                b.iter(|| black_box(functor.apply_object(black_box(obj))))
            },
        );
    }

    group.finish();
}

fn bench_composition_chains(c: &mut Criterion) {
    let mut group = c.benchmark_group("category/composition");
    group.sample_size(50);

    for &chain_length in &[10, 50, 100, 200] {
        let dim = 32;
        let (mut cat, morphisms) = setup_category_with_chain(dim, chain_length);

        group.throughput(Throughput::Elements(chain_length as u64));

        group.bench_with_input(
            BenchmarkId::new("sequential", chain_length),
            &morphisms,
            |b, morphisms| {
                b.iter_batched(
                    || {
                        let (cat, _) = setup_category_with_chain(dim, chain_length);
                        cat
                    },
                    |mut cat| {
                        let mut result = morphisms[0];
                        for &mor in morphisms.iter().skip(1) {
                            result = cat.compose(result, mor).unwrap();
                        }
                        black_box(result)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("chain_compose", chain_length),
            &morphisms,
            |b, morphisms| {
                b.iter_batched(
                    || {
                        let (cat, _) = setup_category_with_chain(dim, chain_length);
                        cat
                    },
                    |mut cat| black_box(cat.compose_chain(morphisms)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_topos_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("category/topos");
    group.sample_size(50);

    for &dim in &[8, 16, 32, 64] {
        group.throughput(Throughput::Elements(dim as u64));

        // Setup for pullback
        group.bench_with_input(
            BenchmarkId::new("pullback", dim),
            &dim,
            |b, &dim| {
                b.iter_batched(
                    || {
                        let mut topos = Topos::new();
                        let a = topos.base_category.add_object(dim);
                        let b = topos.base_category.add_object(dim);
                        let c = topos.base_category.add_object(dim);

                        let mat_f = generate_random_matrix(dim, dim, 42);
                        let mat_g = generate_random_matrix(dim, dim, 43);

                        let f = topos.base_category.add_morphism(a, c, mat_f);
                        let g = topos.base_category.add_morphism(b, c, mat_g);

                        (topos, f, g)
                    },
                    |(mut topos, f, g)| black_box(topos.pullback(f, g)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        // Pushout
        group.bench_with_input(
            BenchmarkId::new("pushout", dim),
            &dim,
            |b, &dim| {
                b.iter_batched(
                    || {
                        let mut topos = Topos::new();
                        let c = topos.base_category.add_object(dim);
                        let a = topos.base_category.add_object(dim);
                        let b = topos.base_category.add_object(dim);

                        let mat_f = generate_random_matrix(dim, dim, 44);
                        let mat_g = generate_random_matrix(dim, dim, 45);

                        let f = topos.base_category.add_morphism(c, a, mat_f);
                        let g = topos.base_category.add_morphism(c, b, mat_g);

                        (topos, f, g)
                    },
                    |(mut topos, f, g)| black_box(topos.pushout(f, g)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        // Exponential
        group.bench_with_input(
            BenchmarkId::new("exponential", dim),
            &dim,
            |b, &dim| {
                b.iter_batched(
                    || {
                        let mut topos = Topos::new();
                        let a = topos.base_category.add_object(dim);
                        let b = topos.base_category.add_object(dim);
                        (topos, a, b)
                    },
                    |(mut topos, a, b)| black_box(topos.exponential(a, b)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_natural_transformation(c: &mut Criterion) {
    let mut group = c.benchmark_group("category/natural_transformation");
    group.sample_size(50);

    for &dim in &[16, 32, 64, 128] {
        let mut nat_trans = NaturalTransformation::new();

        // Add components for multiple objects
        for i in 0..10 {
            let matrix = generate_random_matrix(dim, dim, i * 42);
            nat_trans.add_component(ObjectId(i), matrix);
        }

        let data: Vec<f64> = (0..dim).map(|i| (i as f64).sin()).collect();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(
            BenchmarkId::new("apply_component", dim),
            &(&nat_trans, ObjectId(0), &data),
            |b, (nat_trans, obj, data)| {
                b.iter(|| black_box(nat_trans.apply_at(*obj, black_box(data))))
            },
        );

        // Setup naturality check
        let f = Morphism {
            id: MorphismId(0),
            source: ObjectId(0),
            target: ObjectId(1),
            matrix: Some(generate_random_matrix(dim, dim, 100)),
        };

        let f_prime = Morphism {
            id: MorphismId(1),
            source: ObjectId(0),
            target: ObjectId(1),
            matrix: Some(generate_random_matrix(dim, dim, 101)),
        };

        group.bench_with_input(
            BenchmarkId::new("check_naturality", dim),
            &(&nat_trans, &f, &f_prime),
            |b, (nat_trans, f, f_prime)| {
                b.iter(|| black_box(nat_trans.check_naturality(black_box(f), black_box(f_prime))))
            },
        );
    }

    group.finish();
}

fn bench_matrix_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("category/matrix");
    group.sample_size(50);

    for &dim in &[32, 64, 128, 256] {
        let a = generate_random_matrix(dim, dim, 42);
        let b = generate_random_matrix(dim, dim, 43);
        let v: Vec<f64> = (0..dim).map(|i| (i as f64).sin()).collect();

        group.throughput(Throughput::Elements((dim * dim) as u64));

        group.bench_with_input(
            BenchmarkId::new("multiply", dim),
            &(&a, &b),
            |b, (a, b_mat)| {
                b.iter(|| black_box(matrix_multiply(black_box(a), black_box(b_mat))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("matvec", dim),
            &(&a, &v),
            |b, (a, v)| {
                b.iter(|| black_box(matvec(black_box(a), black_box(v))))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_functor_application,
    bench_composition_chains,
    bench_topos_operations,
    bench_natural_transformation,
    bench_matrix_operations,
);
criterion_main!(benches);
