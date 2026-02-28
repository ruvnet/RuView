//! Thread-local resource pools for proof-checking.
//!
//! Modeled after `ruvector-mincut`'s BfsPool pattern (90%+ hit rate).

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static PROOF_POOL: RefCell<ProofResourcePool> = RefCell::new(ProofResourcePool::new());
}

struct ProofResourcePool {
    envs: Vec<crate::ProofEnvironment>,
    hashmaps: Vec<HashMap<u64, u32>>,
    acquires: u64,
    hits: u64,
}

impl ProofResourcePool {
    fn new() -> Self {
        Self {
            envs: Vec::new(),
            hashmaps: Vec::new(),
            acquires: 0,
            hits: 0,
        }
    }
}

/// Pooled proof resources with auto-return on drop.
pub struct PooledResources {
    pub env: crate::ProofEnvironment,
    pub scratch: HashMap<u64, u32>,
}

impl Drop for PooledResources {
    fn drop(&mut self) {
        let mut env = std::mem::take(&mut self.env);
        env.reset();
        let mut map = std::mem::take(&mut self.scratch);
        map.clear();

        PROOF_POOL.with(|pool| {
            let mut p = pool.borrow_mut();
            p.envs.push(env);
            p.hashmaps.push(map);
        });
    }
}

/// Acquire pooled resources. Auto-returns to pool when dropped.
pub fn acquire() -> PooledResources {
    PROOF_POOL.with(|pool| {
        let mut p = pool.borrow_mut();
        p.acquires += 1;

        let had_env = !p.envs.is_empty();
        let had_map = !p.hashmaps.is_empty();

        let env = p.envs.pop().unwrap_or_else(crate::ProofEnvironment::new);
        let scratch = p.hashmaps.pop().unwrap_or_default();

        if had_env || had_map {
            p.hits += 1;
        }

        PooledResources { env, scratch }
    })
}

/// Get pool statistics: (acquires, hits, hit_rate).
pub fn pool_stats() -> (u64, u64, f64) {
    PROOF_POOL.with(|pool| {
        let p = pool.borrow();
        let rate = if p.acquires == 0 {
            0.0
        } else {
            p.hits as f64 / p.acquires as f64
        };
        (p.acquires, p.hits, rate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_returns() {
        {
            let res = acquire();
            assert!(res.env.symbol_id("Nat").is_some());
        }
        // After drop, pool should have 1 entry
        let (acquires, _, _) = pool_stats();
        assert!(acquires >= 1);
    }

    #[test]
    fn test_pool_reuse() {
        {
            let _r1 = acquire();
        }
        {
            let _r2 = acquire();
        }
        let (acquires, hits, _) = pool_stats();
        assert!(acquires >= 2);
        assert!(hits >= 1, "second acquire should hit pool");
    }

    #[test]
    fn test_pooled_env_is_reset() {
        {
            let mut res = acquire();
            res.env.alloc_term();
            res.env.alloc_term();
        }
        {
            let res = acquire();
            assert_eq!(res.env.terms_allocated(), 0, "pooled env should be reset");
        }
    }
}
