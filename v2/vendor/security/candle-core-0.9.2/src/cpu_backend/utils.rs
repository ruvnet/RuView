/// Helper functions to write CPU kernels.
use crate::backend::BackendStorage;
use crate::{Error, Layout, Result, WithDType};

type C = super::CpuStorage;
pub trait Map1 {
    fn f<T: WithDType>(&self, vs: &[T], layout: &Layout) -> Result<Vec<T>>;

    fn map(&self, vs: &C, layout: &Layout) -> Result<C> {
        match vs {
            C::U8(vs) => Ok(C::U8(self.f(vs, layout)?)),
            C::U32(vs) => Ok(C::U32(self.f(vs, layout)?)),
            C::I16(vs) => Ok(C::I16(self.f(vs, layout)?)),
            C::I32(vs) => Ok(C::I32(self.f(vs, layout)?)),
            C::I64(vs) => Ok(C::I64(self.f(vs, layout)?)),
            C::BF16(vs) => Ok(C::BF16(self.f(vs, layout)?)),
            C::F16(vs) => Ok(C::F16(self.f(vs, layout)?)),
            C::F32(vs) => Ok(C::F32(self.f(vs, layout)?)),
            C::F64(vs) => Ok(C::F64(self.f(vs, layout)?)),
            C::F8E4M3(vs) => Ok(C::F8E4M3(self.f(vs, layout)?)),
            // Dummy types don't support Map1 operations
            C::F6E2M3(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1").bt()),
            C::F6E3M2(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1").bt()),
            C::F4(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1").bt()),
            C::F8E8M0(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1").bt()),
        }
    }
}

pub trait Map1Any {
    fn f<T: WithDType, W: Fn(Vec<T>) -> C>(&self, vs: &[T], layout: &Layout, wrap: W) -> Result<C>;

    fn map(&self, vs: &C, layout: &Layout) -> Result<C> {
        match vs {
            C::U8(vs) => Ok(self.f(vs, layout, C::U8)?),
            C::U32(vs) => Ok(self.f(vs, layout, C::U32)?),
            C::I16(vs) => Ok(self.f(vs, layout, C::I16)?),
            C::I32(vs) => Ok(self.f(vs, layout, C::I32)?),
            C::I64(vs) => Ok(self.f(vs, layout, C::I64)?),
            C::BF16(vs) => Ok(self.f(vs, layout, C::BF16)?),
            C::F16(vs) => Ok(self.f(vs, layout, C::F16)?),
            C::F32(vs) => Ok(self.f(vs, layout, C::F32)?),
            C::F64(vs) => Ok(self.f(vs, layout, C::F64)?),
            C::F8E4M3(vs) => Ok(self.f(vs, layout, C::F8E4M3)?),
            // Dummy types don't support Map1Any operations
            C::F6E2M3(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1any").bt()),
            C::F6E3M2(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1any").bt()),
            C::F4(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1any").bt()),
            C::F8E8M0(_) => Err(Error::UnsupportedDTypeForOp(vs.dtype(), "map1any").bt()),
        }
    }
}

pub trait Map2 {
    const OP: &'static str;
    fn f<T: WithDType>(&self, v1: &[T], l1: &Layout, v2: &[T], l2: &Layout) -> Result<Vec<T>>;

    fn map(&self, v1: &C, l1: &Layout, v2: &C, l2: &Layout) -> Result<C> {
        match (v1, v2) {
            (C::U8(v1), C::U8(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::U32(v1), C::U32(v2)) => Ok(C::U32(self.f(v1, l1, v2, l2)?)),
            (C::I16(v1), C::I16(v2)) => Ok(C::I16(self.f(v1, l1, v2, l2)?)),
            (C::I32(v1), C::I32(v2)) => Ok(C::I32(self.f(v1, l1, v2, l2)?)),
            (C::I64(v1), C::I64(v2)) => Ok(C::I64(self.f(v1, l1, v2, l2)?)),
            (C::BF16(v1), C::BF16(v2)) => Ok(C::BF16(self.f(v1, l1, v2, l2)?)),
            (C::F16(v1), C::F16(v2)) => Ok(C::F16(self.f(v1, l1, v2, l2)?)),
            (C::F32(v1), C::F32(v2)) => Ok(C::F32(self.f(v1, l1, v2, l2)?)),
            (C::F64(v1), C::F64(v2)) => Ok(C::F64(self.f(v1, l1, v2, l2)?)),
            (C::F8E4M3(v1), C::F8E4M3(v2)) => Ok(C::F8E4M3(self.f(v1, l1, v2, l2)?)),
            _ => Err(Error::DTypeMismatchBinaryOp {
                lhs: v1.dtype(),
                rhs: v2.dtype(),
                op: Self::OP,
            }
            .bt()),
        }
    }
}

pub trait Map2InPlace {
    const OP: &'static str;
    fn f<T: WithDType>(&self, v1: &mut [T], l1: &Layout, v2: &[T], l2: &Layout) -> Result<()>;

    fn map(&self, v1: &mut C, l1: &Layout, v2: &C, l2: &Layout) -> Result<()> {
        match (v1, v2) {
            (C::U8(v1), C::U8(v2)) => self.f(v1, l1, v2, l2)?,
            (C::U32(v1), C::U32(v2)) => self.f(v1, l1, v2, l2)?,
            (C::I16(v1), C::I16(v2)) => self.f(v1, l1, v2, l2)?,
            (C::I32(v1), C::I32(v2)) => self.f(v1, l1, v2, l2)?,
            (C::I64(v1), C::I64(v2)) => self.f(v1, l1, v2, l2)?,
            (C::BF16(v1), C::BF16(v2)) => self.f(v1, l1, v2, l2)?,
            (C::F16(v1), C::F16(v2)) => self.f(v1, l1, v2, l2)?,
            (C::F32(v1), C::F32(v2)) => self.f(v1, l1, v2, l2)?,
            (C::F64(v1), C::F64(v2)) => self.f(v1, l1, v2, l2)?,
            (C::F8E4M3(v1), C::F8E4M3(v2)) => self.f(v1, l1, v2, l2)?,
            (v1, v2) => Err(Error::DTypeMismatchBinaryOp {
                lhs: v1.dtype(),
                rhs: v2.dtype(),
                op: Self::OP,
            }
            .bt())?,
        };
        Ok(())
    }
}

pub trait Map2U8 {
    const OP: &'static str;
    fn f<T: WithDType>(&self, v1: &[T], l1: &Layout, v2: &[T], l2: &Layout) -> Result<Vec<u8>>;

    fn map(&self, v1: &C, l1: &Layout, v2: &C, l2: &Layout) -> Result<C> {
        match (v1, v2) {
            (C::U8(v1), C::U8(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::U32(v1), C::U32(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::I16(v1), C::I16(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::I32(v1), C::I32(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::I64(v1), C::I64(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::BF16(v1), C::BF16(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::F16(v1), C::F16(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::F32(v1), C::F32(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::F64(v1), C::F64(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            (C::F8E4M3(v1), C::F8E4M3(v2)) => Ok(C::U8(self.f(v1, l1, v2, l2)?)),
            _ => Err(Error::DTypeMismatchBinaryOp {
                lhs: v1.dtype(),
                rhs: v2.dtype(),
                op: Self::OP,
            }
            .bt()),
        }
    }
}

pub fn binary_map<T: Copy, U: Copy, F: FnMut(T, T) -> U>(
    lhs_l: &Layout,
    rhs_l: &Layout,
    lhs: &[T],
    rhs: &[T],
    mut f: F,
) -> Vec<U> {
    match (lhs_l.contiguous_offsets(), rhs_l.contiguous_offsets()) {
        (Some((o_l1, o_l2)), Some((o_r1, o_r2))) => lhs[o_l1..o_l2]
            .iter()
            .zip(rhs[o_r1..o_r2].iter())
            .map(|(&l, &r)| f(l, r))
            .collect(),
        (Some((o_l1, o_l2)), None) => {
            // TODO: Maybe we want to avoid going through the layout twice.
            match rhs_l.offsets_b() {
                Some(ob) => {
                    let mut i_in_block = 0;
                    let mut i_right_broadcast = 0;
                    lhs[o_l1..o_l2]
                        .iter()
                        .map(|&l| {
                            let r = unsafe { rhs.get_unchecked(i_in_block + ob.start) };
                            i_right_broadcast += 1;
                            if i_right_broadcast >= ob.right_broadcast {
                                i_in_block += 1;
                                i_right_broadcast = 0;
                            }
                            if i_in_block >= ob.len {
                                i_in_block = 0
                            }
                            f(l, *r)
                        })
                        .collect()
                }
                None => lhs_l
                    .strided_index()
                    .zip(rhs_l.strided_index())
                    .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
                    .collect(),
            }
        }
        (None, Some((o_r1, o_r2))) => {
            // TODO: Maybe we want to avoid going through the layout twice.
            match lhs_l.offsets_b() {
                Some(ob) => {
                    let mut i_in_block = 0;
                    let mut i_right_broadcast = 0;
                    rhs[o_r1..o_r2]
                        .iter()
                        .map(|&r| {
                            let l = unsafe { lhs.get_unchecked(i_in_block + ob.start) };
                            i_right_broadcast += 1;
                            if i_right_broadcast >= ob.right_broadcast {
                                i_in_block += 1;
                                i_right_broadcast = 0;
                            }
                            if i_in_block >= ob.len {
                                i_in_block = 0
                            }
                            f(*l, r)
                        })
                        .collect()
                }
                None => lhs_l
                    .strided_index()
                    .zip(rhs_l.strided_index())
                    .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
                    .collect(),
            }
        }
        _ => lhs_l
            .strided_index()
            .zip(rhs_l.strided_index())
            .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
            .collect(),
    }
}

// Similar to binary_map but with vectorized variants.
pub fn binary_map_vec<T: Copy, F: FnMut(T, T) -> T, FV: FnMut(&[T], &[T], &mut [T])>(
    lhs_l: &Layout,
    rhs_l: &Layout,
    lhs: &[T],
    rhs: &[T],
    mut f: F,
    mut f_vec: FV,
) -> Vec<T> {
    let el_count = lhs_l.shape().elem_count();
    match (lhs_l.contiguous_offsets(), rhs_l.contiguous_offsets()) {
        (Some((o_l1, o_l2)), Some((o_r1, o_r2))) => {
            let mut ys: Vec<T> = Vec::with_capacity(el_count);
            let ys_to_set = ys.spare_capacity_mut();
            let ys_to_set = unsafe {
                std::mem::transmute::<&mut [std::mem::MaybeUninit<T>], &mut [T]>(ys_to_set)
            };
            f_vec(&lhs[o_l1..o_l2], &rhs[o_r1..o_r2], ys_to_set);
            // SAFETY: values are all set by f_vec.
            unsafe { ys.set_len(el_count) };
            ys
        }
        (Some((o_l1, o_l2)), None) => match rhs_l.offsets_b() {
            Some(ob) if ob.right_broadcast == 1 => {
                let rhs = &rhs[ob.start..ob.start + ob.len];
                let mut ys: Vec<T> = Vec::with_capacity(el_count);
                let ys_to_set = ys.spare_capacity_mut();
                let ys_to_set = unsafe {
                    std::mem::transmute::<&mut [std::mem::MaybeUninit<T>], &mut [T]>(ys_to_set)
                };
                let mut dst_i = 0;
                for src_i in (o_l1..o_l2).step_by(ob.len) {
                    f_vec(
                        &lhs[src_i..src_i + ob.len],
                        rhs,
                        &mut ys_to_set[dst_i..dst_i + ob.len],
                    );
                    dst_i += ob.len;
                }
                // SAFETY: values are all set by f_vec.
                unsafe { ys.set_len(el_count) };
                ys
            }
            Some(ob) => {
                let rhs = &rhs[ob.start..ob.start + ob.len];
                let mut ys = lhs[o_l1..o_l2].to_vec();
                for idx_l in 0..ob.left_broadcast {
                    let start = idx_l * ob.len * ob.right_broadcast;
                    for (i, &r) in rhs.iter().enumerate() {
                        let start = start + i * ob.right_broadcast;
                        for v in ys[start..start + ob.right_broadcast].iter_mut() {
                            *v = f(*v, r)
                        }
                    }
                }
                ys
            }
            None => lhs_l
                .strided_index()
                .zip(rhs_l.strided_index())
                .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
                .collect(),
        },
        (None, Some((o_r1, o_r2))) => match lhs_l.offsets_b() {
            Some(ob) if ob.right_broadcast == 1 => {
                let lhs = &lhs[ob.start..ob.start + ob.len];
                let mut ys: Vec<T> = Vec::with_capacity(el_count);
                let ys_to_set = ys.spare_capacity_mut();
                let ys_to_set = unsafe {
                    std::mem::transmute::<&mut [std::mem::MaybeUninit<T>], &mut [T]>(ys_to_set)
                };
                let mut dst_i = 0;
                for src_i in (o_r1..o_r2).step_by(ob.len) {
                    f_vec(
                        lhs,
                        &rhs[src_i..src_i + ob.len],
                        &mut ys_to_set[dst_i..dst_i + ob.len],
                    );
                    dst_i += ob.len;
                }
                // SAFETY: values are all set by f_vec.
                unsafe { ys.set_len(el_count) };
                ys
            }
            Some(ob) => {
                let lhs = &lhs[ob.start..ob.start + ob.len];
                let mut ys = rhs[o_r1..o_r2].to_vec();
                for idx_l in 0..ob.left_broadcast {
                    let start = idx_l * ob.len * ob.right_broadcast;
                    for (i, &l) in lhs.iter().enumerate() {
                        let start = start + i * ob.right_broadcast;
                        for v in ys[start..start + ob.right_broadcast].iter_mut() {
                            *v = f(l, *v)
                        }
                    }
                }
                ys
            }
            None => lhs_l
                .strided_index()
                .zip(rhs_l.strided_index())
                .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
                .collect(),
        },
        _ => lhs_l
            .strided_index()
            .zip(rhs_l.strided_index())
            .map(|(lhs_i, rhs_i)| f(lhs[lhs_i], rhs[rhs_i]))
            .collect(),
    }
}

pub fn unary_map<T: Copy, U: Copy, F: FnMut(T) -> U>(
    vs: &[T],
    layout: &Layout,
    mut f: F,
) -> Vec<U> {
    match layout.strided_blocks() {
        crate::StridedBlocks::SingleBlock { start_offset, len } => vs
            [start_offset..start_offset + len]
            .iter()
            .map(|&v| f(v))
            .collect(),
        crate::StridedBlocks::MultipleBlocks {
            block_start_index,
            block_len,
        } => {
            let mut result = Vec::with_capacity(layout.shape().elem_count());
            // Specialize the case where block_len is one to avoid the second loop.
            if block_len == 1 {
                for index in block_start_index {
                    let v = unsafe { vs.get_unchecked(index) };
                    result.push(f(*v))
                }
            } else {
                for index in block_start_index {
                    for offset in 0..block_len {
                        let v = unsafe { vs.get_unchecked(index + offset) };
                        result.push(f(*v))
                    }
                }
            }
            result
        }
    }
}

pub fn unary_map_vec<T: Copy, U: Copy, F: FnMut(T) -> U, FV: FnMut(&[T], &mut [U])>(
    vs: &[T],
    layout: &Layout,
    mut f: F,
    mut f_vec: FV,
) -> Vec<U> {
    match layout.strided_blocks() {
        crate::StridedBlocks::SingleBlock { start_offset, len } => {
            let mut ys: Vec<U> = Vec::with_capacity(len);
            let ys_to_set = ys.spare_capacity_mut();
            let ys_to_set = unsafe {
                std::mem::transmute::<&mut [std::mem::MaybeUninit<U>], &mut [U]>(ys_to_set)
            };
            f_vec(&vs[start_offset..start_offset + len], ys_to_set);
            // SAFETY: values are all set by f_vec.
            unsafe { ys.set_len(len) };
            ys
        }
        crate::StridedBlocks::MultipleBlocks {
            block_start_index,
            block_len,
        } => {
            let el_count = layout.shape().elem_count();
            // Specialize the case where block_len is one to avoid the second loop.
            if block_len == 1 {
                let mut result = Vec::with_capacity(el_count);
                for index in block_start_index {
                    let v = unsafe { vs.get_unchecked(index) };
                    result.push(f(*v))
                }
                result
            } else {
                let mut ys: Vec<U> = Vec::with_capacity(el_count);
                let ys_to_set = ys.spare_capacity_mut();
                let ys_to_set = unsafe {
                    std::mem::transmute::<&mut [std::mem::MaybeUninit<U>], &mut [U]>(ys_to_set)
                };
                let mut dst_index = 0;
                for src_index in block_start_index {
                    let vs = &vs[src_index..src_index + block_len];
                    let ys = &mut ys_to_set[dst_index..dst_index + block_len];
                    f_vec(vs, ys);
                    dst_index += block_len;
                }
                // SAFETY: values are all set by f_vec.
                unsafe { ys.set_len(el_count) };
                ys
            }
        }
    }
}
