pub trait VecOps: num_traits::NumAssign + Copy {
    fn min(self, rhs: Self) -> Self;
    fn max(self, rhs: Self) -> Self;

    /// Dot-product of two vectors.
    ///
    /// # Safety
    ///
    /// The length of `lhs` and `rhs` have to be at least `len`. `res` has to point to a valid
    /// element.
    #[inline(always)]
    unsafe fn vec_dot(lhs: *const Self, rhs: *const Self, res: *mut Self, len: usize) {
        *res = Self::zero();
        for i in 0..len {
            *res += *lhs.add(i) * *rhs.add(i)
        }
    }

    /// Sum of all elements in a vector.
    ///
    /// # Safety
    ///
    /// The length of `xs` must be at least `len`. `res` has to point to a valid
    /// element.
    #[inline(always)]
    unsafe fn vec_reduce_sum(xs: *const Self, res: *mut Self, len: usize) {
        *res = Self::zero();
        for i in 0..len {
            *res += *xs.add(i)
        }
    }

    /// Maximum element in a non-empty vector.
    ///
    /// # Safety
    ///
    /// The length of `xs` must be at least `len` and positive. `res` has to point to a valid
    /// element.
    #[inline(always)]
    unsafe fn vec_reduce_max(xs: *const Self, res: *mut Self, len: usize) {
        *res = *xs;
        for i in 1..len {
            *res = (*res).max(*xs.add(i))
        }
    }

    /// Minimum element in a non-empty vector.
    ///
    /// # Safety
    ///
    /// The length of `xs` must be at least `len` and positive. `res` has to point to a valid
    /// element.
    #[inline(always)]
    unsafe fn vec_reduce_min(xs: *const Self, res: *mut Self, len: usize) {
        *res = *xs;
        for i in 1..len {
            *res = (*res).min(*xs.add(i))
        }
    }
}

impl VecOps for f32 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::max(self, other)
    }

    #[inline(always)]
    unsafe fn vec_dot(lhs: *const Self, rhs: *const Self, res: *mut Self, len: usize) {
        super::vec_dot_f32(lhs, rhs, res, len)
    }

    #[inline(always)]
    unsafe fn vec_reduce_sum(xs: *const Self, res: *mut Self, len: usize) {
        super::vec_sum(xs, res, len)
    }
}

impl VecOps for half::f16 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::max(self, other)
    }

    #[inline(always)]
    unsafe fn vec_dot(lhs: *const Self, rhs: *const Self, res: *mut Self, len: usize) {
        let mut res_f32 = 0f32;
        super::vec_dot_f16(lhs, rhs, &mut res_f32, len);
        *res = half::f16::from_f32(res_f32);
    }
}

impl VecOps for f64 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::max(self, other)
    }
}
impl VecOps for half::bf16 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::max(self, other)
    }

    #[inline(always)]
    unsafe fn vec_dot(lhs: *const Self, rhs: *const Self, res: *mut Self, len: usize) {
        let mut res_f32 = 0f32;
        super::vec_dot_bf16(lhs, rhs, &mut res_f32, len);
        *res = half::bf16::from_f32(res_f32);
    }
}
impl VecOps for u8 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        <Self as Ord>::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        <Self as Ord>::max(self, other)
    }
}
impl VecOps for u32 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        <Self as Ord>::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        <Self as Ord>::max(self, other)
    }
}
impl VecOps for i16 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        <Self as Ord>::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        <Self as Ord>::max(self, other)
    }
}
impl VecOps for i32 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        <Self as Ord>::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        <Self as Ord>::max(self, other)
    }
}
impl VecOps for i64 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        <Self as Ord>::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        <Self as Ord>::max(self, other)
    }
}

impl VecOps for float8::F8E4M3 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::min(self, other)
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::max(self, other)
    }
}

#[inline(always)]
pub fn par_for_each(n_threads: usize, func: impl Fn(usize) + Send + Sync) {
    if n_threads == 1 {
        func(0)
    } else {
        rayon::scope(|s| {
            for thread_idx in 0..n_threads {
                let func = &func;
                s.spawn(move |_| func(thread_idx));
            }
        })
    }
}

#[inline(always)]
pub fn par_range(lo: usize, up: usize, n_threads: usize, func: impl Fn(usize) + Send + Sync) {
    if n_threads == 1 {
        for i in lo..up {
            func(i)
        }
    } else {
        rayon::scope(|s| {
            for thread_idx in 0..n_threads {
                let func = &func;
                s.spawn(move |_| {
                    for i in (thread_idx..up).step_by(n_threads) {
                        func(i)
                    }
                });
            }
        })
    }
}
