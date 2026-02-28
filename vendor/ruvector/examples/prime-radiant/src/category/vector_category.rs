//! The Category of Vector Spaces (Vect_k)
//!
//! This module implements the category of finite-dimensional vector spaces
//! over a field k (typically R or C), where:
//! - Objects are vector spaces of given dimension
//! - Morphisms are linear transformations (matrices)
//! - Composition is matrix multiplication
//! - Identity is the identity matrix

use super::{Category, CategoryWithMono, CategoryWithProducts};
use super::object::{Object, ObjectData};
use super::morphism::{Morphism, MorphismData};
use crate::{ObjectId, MorphismId, CategoryError, Result};
use dashmap::DashMap;
use std::sync::Arc;

/// The category of finite-dimensional vector spaces
///
/// Objects are vector spaces represented by their dimension.
/// Morphisms are linear transformations represented as matrices.
#[derive(Debug)]
pub struct VectorCategory {
    /// The base dimension for embeddings
    base_dimension: usize,
    /// Objects in the category
    objects: Arc<DashMap<ObjectId, Object<ObjectData>>>,
    /// Morphisms in the category
    morphisms: Arc<DashMap<MorphismId, Morphism<MorphismData>>>,
    /// Identity morphisms cache
    identities: Arc<DashMap<ObjectId, MorphismId>>,
}

impl VectorCategory {
    /// Creates a new category of vector spaces
    pub fn new(base_dimension: usize) -> Self {
        Self {
            base_dimension,
            objects: Arc::new(DashMap::new()),
            morphisms: Arc::new(DashMap::new()),
            identities: Arc::new(DashMap::new()),
        }
    }

    /// Gets the base dimension
    pub fn dimension(&self) -> usize {
        self.base_dimension
    }

    /// Adds a vector space of given dimension
    pub fn add_vector_space(&self, dimension: usize) -> Object<ObjectData> {
        let obj = Object::new(ObjectData::VectorSpace(dimension));
        let id = obj.id;
        self.objects.insert(id, obj.clone());

        // Create identity matrix morphism
        let identity_matrix = Self::identity_matrix(dimension);
        let identity = Morphism::identity(id, MorphismData::LinearMap(identity_matrix));
        let identity_id = identity.id;
        self.morphisms.insert(identity_id, identity);
        self.identities.insert(id, identity_id);

        obj
    }

    /// Adds a linear map between vector spaces
    ///
    /// The matrix should be rows x cols where:
    /// - rows = dimension of codomain
    /// - cols = dimension of domain
    pub fn add_linear_map(
        &self,
        domain: &Object<ObjectData>,
        codomain: &Object<ObjectData>,
        matrix: Vec<Vec<f64>>,
    ) -> Result<Morphism<MorphismData>> {
        // Validate dimensions
        let (dom_dim, cod_dim) = match (&domain.data, &codomain.data) {
            (ObjectData::VectorSpace(d), ObjectData::VectorSpace(c)) => (*d, *c),
            _ => return Err(CategoryError::Internal("Expected vector spaces".to_string())),
        };

        if matrix.len() != cod_dim {
            return Err(CategoryError::InvalidDimension {
                expected: cod_dim,
                got: matrix.len(),
            });
        }

        for row in &matrix {
            if row.len() != dom_dim {
                return Err(CategoryError::InvalidDimension {
                    expected: dom_dim,
                    got: row.len(),
                });
            }
        }

        let mor = Morphism::new(
            domain.id,
            codomain.id,
            MorphismData::LinearMap(matrix),
        );
        let id = mor.id;
        self.morphisms.insert(id, mor.clone());

        Ok(mor)
    }

    /// Gets an object by ID
    pub fn get_object(&self, id: &ObjectId) -> Option<Object<ObjectData>> {
        self.objects.get(id).map(|e| e.clone())
    }

    /// Gets a morphism by ID
    pub fn get_morphism(&self, id: &MorphismId) -> Option<Morphism<MorphismData>> {
        self.morphisms.get(id).map(|e| e.clone())
    }

    /// Creates an identity matrix
    fn identity_matrix(dim: usize) -> Vec<Vec<f64>> {
        (0..dim)
            .map(|i| {
                (0..dim)
                    .map(|j| if i == j { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect()
    }

    /// Multiplies two matrices (B * A for composition A then B)
    fn matrix_multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
        if a.is_empty() || b.is_empty() {
            return Some(vec![]);
        }

        let a_rows = a.len();
        let a_cols = a[0].len();
        let b_rows = b.len();
        let b_cols = b[0].len();

        // For B * A, we need a_cols == b_rows
        if a_cols != b_rows {
            return None;
        }

        let mut result = vec![vec![0.0; b_cols]; a_rows];
        for i in 0..a_rows {
            for j in 0..b_cols {
                for k in 0..a_cols {
                    result[i][j] += a[i][k] * b[k][j];
                }
            }
        }

        Some(result)
    }

    /// Computes the rank of a matrix
    fn matrix_rank(matrix: &[Vec<f64>]) -> usize {
        if matrix.is_empty() || matrix[0].is_empty() {
            return 0;
        }

        // Simple rank computation via row echelon form
        let mut m: Vec<Vec<f64>> = matrix.to_vec();
        let rows = m.len();
        let cols = m[0].len();

        let mut rank = 0;
        let mut col = 0;

        while rank < rows && col < cols {
            // Find pivot
            let mut max_row = rank;
            for i in (rank + 1)..rows {
                if m[i][col].abs() > m[max_row][col].abs() {
                    max_row = i;
                }
            }

            if m[max_row][col].abs() < 1e-10 {
                col += 1;
                continue;
            }

            // Swap rows
            m.swap(rank, max_row);

            // Eliminate
            for i in (rank + 1)..rows {
                let factor = m[i][col] / m[rank][col];
                for j in col..cols {
                    m[i][j] -= factor * m[rank][j];
                }
            }

            rank += 1;
            col += 1;
        }

        rank
    }

    /// Computes matrix determinant (for square matrices)
    fn matrix_determinant(matrix: &[Vec<f64>]) -> Option<f64> {
        let n = matrix.len();
        if n == 0 {
            return Some(1.0);
        }
        if matrix.iter().any(|row| row.len() != n) {
            return None; // Not square
        }

        if n == 1 {
            return Some(matrix[0][0]);
        }

        if n == 2 {
            return Some(matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]);
        }

        // LU decomposition for larger matrices
        let mut m: Vec<Vec<f64>> = matrix.to_vec();
        let mut det = 1.0;

        for i in 0..n {
            // Find pivot
            let mut max_row = i;
            for k in (i + 1)..n {
                if m[k][i].abs() > m[max_row][i].abs() {
                    max_row = k;
                }
            }

            if m[max_row][i].abs() < 1e-10 {
                return Some(0.0);
            }

            if max_row != i {
                m.swap(i, max_row);
                det *= -1.0;
            }

            det *= m[i][i];

            for k in (i + 1)..n {
                let factor = m[k][i] / m[i][i];
                for j in i..n {
                    m[k][j] -= factor * m[i][j];
                }
            }
        }

        Some(det)
    }

    /// Computes matrix inverse
    fn matrix_inverse(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
        let n = matrix.len();
        if n == 0 || matrix.iter().any(|row| row.len() != n) {
            return None;
        }

        // Augmented matrix [A | I]
        let mut aug: Vec<Vec<f64>> = matrix
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut new_row = row.clone();
                new_row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
                new_row
            })
            .collect();

        // Gaussian elimination
        for i in 0..n {
            // Find pivot
            let mut max_row = i;
            for k in (i + 1)..n {
                if aug[k][i].abs() > aug[max_row][i].abs() {
                    max_row = k;
                }
            }

            if aug[max_row][i].abs() < 1e-10 {
                return None; // Singular
            }

            aug.swap(i, max_row);

            // Scale row
            let scale = aug[i][i];
            for j in 0..(2 * n) {
                aug[i][j] /= scale;
            }

            // Eliminate column
            for k in 0..n {
                if k != i {
                    let factor = aug[k][i];
                    for j in 0..(2 * n) {
                        aug[k][j] -= factor * aug[i][j];
                    }
                }
            }
        }

        // Extract inverse
        Some(aug.into_iter().map(|row| row[n..].to_vec()).collect())
    }
}

impl Clone for VectorCategory {
    fn clone(&self) -> Self {
        let new_cat = Self::new(self.base_dimension);
        for entry in self.objects.iter() {
            new_cat.objects.insert(*entry.key(), entry.value().clone());
        }
        for entry in self.morphisms.iter() {
            new_cat.morphisms.insert(*entry.key(), entry.value().clone());
        }
        for entry in self.identities.iter() {
            new_cat.identities.insert(*entry.key(), *entry.value());
        }
        new_cat
    }
}

impl Category for VectorCategory {
    type Object = Object<ObjectData>;
    type Morphism = Morphism<MorphismData>;

    fn identity(&self, obj: &Self::Object) -> Option<Self::Morphism> {
        self.identities
            .get(&obj.id)
            .and_then(|id| self.morphisms.get(&id).map(|m| m.clone()))
    }

    fn compose(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism> {
        // Check composability: cod(f) = dom(g)
        if f.codomain != g.domain {
            return None;
        }

        // Handle identity cases
        if f.is_identity {
            return Some(g.clone());
        }
        if g.is_identity {
            return Some(f.clone());
        }

        // Compose the matrices
        let composed_data = match (&f.data, &g.data) {
            (MorphismData::LinearMap(f_mat), MorphismData::LinearMap(g_mat)) => {
                // (g . f)(v) = g(f(v)) = G * (F * v) = (G * F) * v
                let composed_matrix = Self::matrix_multiply(g_mat, f_mat)?;
                MorphismData::LinearMap(composed_matrix)
            }
            _ => MorphismData::compose(f.data.clone(), g.data.clone()),
        };

        let composed = Morphism::new(f.domain, g.codomain, composed_data);
        self.morphisms.insert(composed.id, composed.clone());

        Some(composed)
    }

    fn domain(&self, mor: &Self::Morphism) -> Self::Object {
        self.get_object(&mor.domain).unwrap()
    }

    fn codomain(&self, mor: &Self::Morphism) -> Self::Object {
        self.get_object(&mor.codomain).unwrap()
    }

    fn is_identity(&self, mor: &Self::Morphism) -> bool {
        if mor.is_identity {
            return true;
        }

        match &mor.data {
            MorphismData::LinearMap(matrix) => {
                let n = matrix.len();
                if n == 0 {
                    return true;
                }
                matrix.iter().enumerate().all(|(i, row)| {
                    row.len() == n
                        && row
                            .iter()
                            .enumerate()
                            .all(|(j, &v)| (v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-10)
                })
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn verify_laws(&self) -> bool {
        // Verify identity laws
        for mor_entry in self.morphisms.iter() {
            let mor = mor_entry.value();
            if mor.is_identity {
                continue;
            }

            if let (Some(id_dom), Some(id_cod)) = (
                self.identity(&self.domain(mor)),
                self.identity(&self.codomain(mor)),
            ) {
                // Check id_cod . f = f
                if let Some(composed) = self.compose(mor, &id_cod) {
                    if !self.morphisms_equal(&composed, mor) {
                        return false;
                    }
                }

                // Check f . id_dom = f
                if let Some(composed) = self.compose(&id_dom, mor) {
                    if !self.morphisms_equal(&composed, mor) {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn objects(&self) -> Vec<Self::Object> {
        self.objects.iter().map(|e| e.value().clone()).collect()
    }

    fn morphisms(&self) -> Vec<Self::Morphism> {
        self.morphisms.iter().map(|e| e.value().clone()).collect()
    }

    fn contains_object(&self, obj: &Self::Object) -> bool {
        self.objects.contains_key(&obj.id)
    }

    fn contains_morphism(&self, mor: &Self::Morphism) -> bool {
        self.morphisms.contains_key(&mor.id)
    }
}

impl VectorCategory {
    /// Checks if two morphisms have equal matrix data
    fn morphisms_equal(&self, a: &Morphism<MorphismData>, b: &Morphism<MorphismData>) -> bool {
        match (&a.data, &b.data) {
            (MorphismData::LinearMap(m1), MorphismData::LinearMap(m2)) => {
                if m1.len() != m2.len() {
                    return false;
                }
                m1.iter().zip(m2.iter()).all(|(r1, r2)| {
                    r1.len() == r2.len()
                        && r1.iter().zip(r2.iter()).all(|(v1, v2)| (v1 - v2).abs() < 1e-10)
                })
            }
            _ => false,
        }
    }
}

impl CategoryWithMono for VectorCategory {
    fn is_monomorphism(&self, mor: &Self::Morphism) -> bool {
        // A linear map is mono iff it has full column rank (injective)
        match &mor.data {
            MorphismData::LinearMap(matrix) => {
                if matrix.is_empty() {
                    return true;
                }
                let dom_dim = matrix[0].len();
                Self::matrix_rank(matrix) == dom_dim
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn is_epimorphism(&self, mor: &Self::Morphism) -> bool {
        // A linear map is epi iff it has full row rank (surjective)
        match &mor.data {
            MorphismData::LinearMap(matrix) => {
                let cod_dim = matrix.len();
                Self::matrix_rank(matrix) == cod_dim
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn is_isomorphism(&self, mor: &Self::Morphism) -> bool {
        // Square matrix with full rank
        match &mor.data {
            MorphismData::LinearMap(matrix) => {
                let rows = matrix.len();
                let cols = if rows > 0 { matrix[0].len() } else { 0 };
                rows == cols && Self::matrix_determinant(matrix).map(|d| d.abs() > 1e-10).unwrap_or(false)
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn inverse(&self, mor: &Self::Morphism) -> Option<Self::Morphism> {
        if !self.is_isomorphism(mor) {
            return None;
        }

        match &mor.data {
            MorphismData::LinearMap(matrix) => {
                let inv_matrix = Self::matrix_inverse(matrix)?;
                let inverse = Morphism::new(
                    mor.codomain,
                    mor.domain,
                    MorphismData::LinearMap(inv_matrix),
                );
                self.morphisms.insert(inverse.id, inverse.clone());
                Some(inverse)
            }
            MorphismData::Identity => Some(mor.clone()),
            _ => None,
        }
    }
}

impl CategoryWithProducts for VectorCategory {
    fn product(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object> {
        // Product of vector spaces is direct sum (same dimension = sum)
        let (a_dim, b_dim) = match (&a.data, &b.data) {
            (ObjectData::VectorSpace(d1), ObjectData::VectorSpace(d2)) => (*d1, *d2),
            _ => return None,
        };

        let product = self.add_vector_space(a_dim + b_dim);
        Some(product)
    }

    fn proj1(&self, product: &Self::Object) -> Option<Self::Morphism> {
        // For this we'd need to track which objects were combined
        // Simplified: create projection to first half
        match &product.data {
            ObjectData::VectorSpace(total_dim) => {
                let half_dim = total_dim / 2;
                if half_dim == 0 {
                    return None;
                }

                let target = self.add_vector_space(half_dim);

                // Projection matrix: [I_n | 0]
                let mut matrix = vec![vec![0.0; *total_dim]; half_dim];
                for i in 0..half_dim {
                    matrix[i][i] = 1.0;
                }

                let proj = Morphism::new(
                    product.id,
                    target.id,
                    MorphismData::LinearMap(matrix),
                );
                self.morphisms.insert(proj.id, proj.clone());
                Some(proj)
            }
            _ => None,
        }
    }

    fn proj2(&self, product: &Self::Object) -> Option<Self::Morphism> {
        match &product.data {
            ObjectData::VectorSpace(total_dim) => {
                let half_dim = total_dim / 2;
                let second_half = total_dim - half_dim;
                if second_half == 0 {
                    return None;
                }

                let target = self.add_vector_space(second_half);

                // Projection matrix: [0 | I_m]
                let mut matrix = vec![vec![0.0; *total_dim]; second_half];
                for i in 0..second_half {
                    matrix[i][half_dim + i] = 1.0;
                }

                let proj = Morphism::new(
                    product.id,
                    target.id,
                    MorphismData::LinearMap(matrix),
                );
                self.morphisms.insert(proj.id, proj.clone());
                Some(proj)
            }
            _ => None,
        }
    }

    fn pair(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism> {
        if f.domain != g.domain {
            return None;
        }

        // <f, g>(v) = (f(v), g(v))
        // Matrix: [F; G] (vertical stack)
        match (&f.data, &g.data) {
            (MorphismData::LinearMap(f_mat), MorphismData::LinearMap(g_mat)) => {
                let mut combined = f_mat.clone();
                combined.extend(g_mat.clone());

                let product = self.product(&self.codomain(f), &self.codomain(g))?;

                let paired = Morphism::new(
                    f.domain,
                    product.id,
                    MorphismData::LinearMap(combined),
                );
                self.morphisms.insert(paired.id, paired.clone());
                Some(paired)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_category_basic() {
        let cat = VectorCategory::new(768);

        let v2 = cat.add_vector_space(2);
        let v3 = cat.add_vector_space(3);

        assert!(cat.contains_object(&v2));
        assert!(cat.contains_object(&v3));
    }

    #[test]
    fn test_identity_matrix() {
        let cat = VectorCategory::new(768);
        let v = cat.add_vector_space(3);

        let id = cat.identity(&v).unwrap();
        assert!(cat.is_identity(&id));
    }

    #[test]
    fn test_linear_map() {
        let cat = VectorCategory::new(768);

        let v2 = cat.add_vector_space(2);
        let v3 = cat.add_vector_space(3);

        // 3x2 matrix
        let matrix = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];

        let mor = cat.add_linear_map(&v2, &v3, matrix).unwrap();

        assert!(!cat.is_identity(&mor));
        assert!(cat.is_monomorphism(&mor));
        assert!(!cat.is_epimorphism(&mor));
    }

    #[test]
    fn test_matrix_composition() {
        let cat = VectorCategory::new(768);

        let v2 = cat.add_vector_space(2);
        let v3 = cat.add_vector_space(3);

        // f: R^2 -> R^3
        let f_matrix = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];
        let f = cat.add_linear_map(&v2, &v3, f_matrix).unwrap();

        // g: R^3 -> R^2
        let g_matrix = vec![
            vec![1.0, 1.0, 0.0],
            vec![0.0, 1.0, 1.0],
        ];
        let g = cat.add_linear_map(&v3, &v2, g_matrix).unwrap();

        // g.f: R^2 -> R^2
        let gf = cat.compose(&f, &g).unwrap();

        // g * f should be a 2x2 matrix
        match &gf.data {
            MorphismData::LinearMap(matrix) => {
                assert_eq!(matrix.len(), 2);
                assert_eq!(matrix[0].len(), 2);
            }
            _ => panic!("Expected LinearMap"),
        }
    }

    #[test]
    fn test_isomorphism_inverse() {
        let cat = VectorCategory::new(768);

        let v2 = cat.add_vector_space(2);

        // Rotation matrix (orthogonal, thus invertible)
        let angle = std::f64::consts::PI / 4.0;
        let cos = angle.cos();
        let sin = angle.sin();

        let rotation = vec![
            vec![cos, -sin],
            vec![sin, cos],
        ];

        let mor = cat.add_linear_map(&v2, &v2, rotation).unwrap();

        assert!(cat.is_isomorphism(&mor));

        let inv = cat.inverse(&mor).unwrap();

        // Compose should give identity
        let composed = cat.compose(&mor, &inv).unwrap();
        assert!(cat.is_identity(&composed));
    }
}
