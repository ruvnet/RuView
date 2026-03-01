//! Product Quantization - 8-32x Memory Compression

use heapless::Vec as HVec;

pub const MAX_SUBQUANTIZERS: usize = 8;
pub const MAX_CODEBOOK_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, Default)]
pub struct PQConfig {
    pub num_subquantizers: usize,
    pub codebook_size: usize,
    pub subvec_dim: usize,
    pub dim: usize,
}

impl PQConfig {
    pub fn new(dim: usize, num_sub: usize) -> Self {
        Self {
            num_subquantizers: num_sub,
            codebook_size: 16,
            subvec_dim: dim / num_sub,
            dim,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PQCode<const M: usize> {
    pub codes: HVec<u8, M>,
}

impl<const M: usize> PQCode<M> {
    pub fn from_codes(codes: &[u8]) -> crate::Result<Self> {
        let mut code_vec = HVec::new();
        for &c in codes {
            code_vec.push(c).map_err(|_| crate::Error::BufferOverflow)?;
        }
        Ok(Self { codes: code_vec })
    }

    #[inline]
    pub fn get_code(&self, i: usize) -> u8 {
        self.codes.get(i).copied().unwrap_or(0)
    }
}

pub struct ProductQuantizer<const M: usize, const K: usize, const D: usize> {
    codebooks: HVec<i8, { 8 * 16 * 8 }>,
    config: PQConfig,
}

impl<const M: usize, const K: usize, const D: usize> ProductQuantizer<M, K, D> {
    pub fn random(config: PQConfig, seed: u32) -> crate::Result<Self> {
        let total = config.num_subquantizers * config.codebook_size * config.subvec_dim;
        let mut codebooks = HVec::new();
        let mut rng = seed;

        for _ in 0..total {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let val = (((rng >> 16) & 0xFF) as i16 - 128) as i8;
            codebooks.push(val).map_err(|_| crate::Error::BufferOverflow)?;
        }
        Ok(Self { codebooks, config })
    }

    #[inline]
    fn get_centroid(&self, m: usize, k: usize) -> &[i8] {
        let d = self.config.subvec_dim;
        let kk = self.config.codebook_size;
        let start = m * kk * d + k * d;
        &self.codebooks[start..start + d]
    }

    pub fn encode(&self, vector: &[i8]) -> crate::Result<PQCode<M>> {
        if vector.len() != self.config.dim {
            return Err(crate::Error::InvalidModel("Dimension mismatch"));
        }
        let mut codes = HVec::new();
        let d = self.config.subvec_dim;

        for m in 0..self.config.num_subquantizers {
            let subvec = &vector[m * d..(m + 1) * d];
            let mut best_code = 0u8;
            let mut best_dist = i32::MAX;

            for k in 0..self.config.codebook_size {
                let dist = Self::l2_squared(subvec, self.get_centroid(m, k));
                if dist < best_dist {
                    best_dist = dist;
                    best_code = k as u8;
                }
            }
            codes.push(best_code).map_err(|_| crate::Error::BufferOverflow)?;
        }
        Ok(PQCode { codes })
    }

    pub fn asymmetric_distance(&self, query: &[i8], code: &PQCode<M>) -> i32 {
        let d = self.config.subvec_dim;
        let mut total: i32 = 0;
        for m in 0..self.config.num_subquantizers {
            let query_sub = &query[m * d..(m + 1) * d];
            let k = code.get_code(m) as usize;
            total += Self::l2_squared(query_sub, self.get_centroid(m, k));
        }
        total
    }

    pub fn build_distance_table(&self, query: &[i8]) -> PQDistanceTable<M, K> {
        let mut table = PQDistanceTable::new();
        let d = self.config.subvec_dim;
        for m in 0..self.config.num_subquantizers {
            let query_sub = &query[m * d..(m + 1) * d];
            for k in 0..self.config.codebook_size {
                let dist = Self::l2_squared(query_sub, self.get_centroid(m, k));
                table.set(m, k, dist);
            }
        }
        table
    }

    #[inline]
    fn l2_squared(a: &[i8], b: &[i8]) -> i32 {
        a.iter().zip(b.iter()).map(|(&x, &y)| {
            let diff = x as i32 - y as i32;
            diff * diff
        }).sum()
    }

    pub fn compression_ratio(&self) -> f32 {
        self.config.dim as f32 / self.config.num_subquantizers as f32
    }
}

pub struct PQDistanceTable<const M: usize, const K: usize> {
    distances: [i32; 128],
}

impl<const M: usize, const K: usize> PQDistanceTable<M, K> {
    pub fn new() -> Self { Self { distances: [0; 128] } }
    #[inline]
    pub fn get(&self, m: usize, k: usize) -> i32 { self.distances[m * K + k] }
    #[inline]
    pub fn set(&mut self, m: usize, k: usize, dist: i32) { self.distances[m * K + k] = dist; }
}

impl<const M: usize, const K: usize> Default for PQDistanceTable<M, K> {
    fn default() -> Self { Self::new() }
}
