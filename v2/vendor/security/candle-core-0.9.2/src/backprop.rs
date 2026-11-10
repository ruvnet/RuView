//! Methods for backpropagation of gradients.
use crate::op::{BinaryOp, Op, ReduceOp, UnaryOp};
use crate::{Error, Result, Tensor, TensorId};
use std::collections::HashMap;

// arg has been reduced to node via reduce_dims, expand it back to arg.
// This has to handle keepdims.
fn broadcast_back(arg: &Tensor, node: &Tensor, reduced_dims: &[usize]) -> Result<Tensor> {
    if arg.rank() == node.rank() {
        // keepdim = true
        node.broadcast_as(arg.shape())
    } else {
        // keepdim = false
        // first expand the reduced dims.
        node.reshape(reduced_dims)?.broadcast_as(arg.shape())
    }
}

thread_local! {
    static CANDLE_GRAD_DO_NOT_DETACH: bool = {
        match std::env::var("CANDLE_GRAD_DO_NOT_DETACH") {
            Ok(s) => {
                !s.is_empty() && s != "0"
            },
            Err(_) => false,
        }
    }
}

impl Tensor {
    /// Return all the nodes that lead to this value in a topologically sorted vec, the first
    /// elements having dependencies on the latter ones, e.g. the first element if any is the
    /// argument.
    /// This assumes that the op graph is a DAG.
    pub fn sorted_nodes(&self) -> Vec<&Tensor> {
        // The vec of sorted nodes is passed as an owned value rather than a mutable reference
        // to get around some lifetime limitations.
        fn walk<'a>(
            node: &'a Tensor,
            nodes: Vec<&'a Tensor>,
            already_seen: &mut HashMap<TensorId, bool>,
        ) -> (bool, Vec<&'a Tensor>) {
            if let Some(&tg) = already_seen.get(&node.id()) {
                return (tg, nodes);
            }
            let mut track_grad = false;
            let mut nodes = if node.is_variable() {
                // Do not call recursively on the "leaf" nodes.
                track_grad = true;
                nodes
            } else if node.dtype().is_int() {
                nodes
            } else if let Some(op) = node.op() {
                match op {
                    Op::IndexAdd(t1, t2, t3, _)
                    | Op::Scatter(t1, t2, t3, _)
                    | Op::ScatterAdd(t1, t2, t3, _)
                    | Op::CustomOp3(t1, t2, t3, _)
                    | Op::WhereCond(t1, t2, t3) => {
                        let (tg, nodes) = walk(t1, nodes, already_seen);
                        track_grad |= tg;
                        let (tg, nodes) = walk(t2, nodes, already_seen);
                        track_grad |= tg;
                        let (tg, nodes) = walk(t3, nodes, already_seen);
                        track_grad |= tg;
                        nodes
                    }
                    Op::Conv1D {
                        arg: lhs,
                        kernel: rhs,
                        ..
                    }
                    | Op::ConvTranspose1D {
                        arg: lhs,
                        kernel: rhs,
                        ..
                    }
                    | Op::Conv2D {
                        arg: lhs,
                        kernel: rhs,
                        ..
                    }
                    | Op::ConvTranspose2D {
                        arg: lhs,
                        kernel: rhs,
                        ..
                    }
                    | Op::CustomOp2(lhs, rhs, _)
                    | Op::Binary(lhs, rhs, _)
                    | Op::Gather(lhs, rhs, _)
                    | Op::IndexSelect(lhs, rhs, _)
                    | Op::Matmul(lhs, rhs)
                    | Op::SliceScatter0(lhs, rhs, _) => {
                        let (tg, nodes) = walk(lhs, nodes, already_seen);
                        track_grad |= tg;
                        let (tg, nodes) = walk(rhs, nodes, already_seen);
                        track_grad |= tg;
                        nodes
                    }
                    Op::Cat(args, _) => args.iter().fold(nodes, |nodes, arg| {
                        let (tg, nodes) = walk(arg, nodes, already_seen);
                        track_grad |= tg;
                        nodes
                    }),
                    Op::Affine { arg, mul, .. } => {
                        if *mul == 0. {
                            nodes
                        } else {
                            let (tg, nodes) = walk(arg, nodes, already_seen);
                            track_grad |= tg;
                            nodes
                        }
                    }
                    Op::Unary(_node, UnaryOp::Ceil)
                    | Op::Unary(_node, UnaryOp::Floor)
                    | Op::Unary(_node, UnaryOp::Round)
                    | Op::Unary(_node, UnaryOp::Sign) => nodes,
                    Op::Reshape(node)
                    | Op::UpsampleNearest1D { arg: node, .. }
                    | Op::UpsampleNearest2D { arg: node, .. }
                    | Op::UpsampleBilinear2D { arg: node, .. }
                    | Op::AvgPool2D { arg: node, .. }
                    | Op::MaxPool2D { arg: node, .. }
                    | Op::Copy(node)
                    | Op::Broadcast(node)
                    | Op::Cmp(node, _)
                    | Op::Reduce(node, ReduceOp::Min | ReduceOp::Sum | ReduceOp::Max, _)
                    | Op::ToDevice(node)
                    | Op::Transpose(node, _, _)
                    | Op::Permute(node, _)
                    | Op::Narrow(node, _, _, _)
                    | Op::Unary(node, _)
                    | Op::Elu(node, _)
                    | Op::Powf(node, _)
                    | Op::CustomOp1(node, _) => {
                        let (tg, nodes) = walk(node, nodes, already_seen);
                        track_grad |= tg;
                        nodes
                    }
                    Op::ToDType(node) => {
                        if node.dtype().is_float() {
                            let (tg, nodes) = walk(node, nodes, already_seen);
                            track_grad |= tg;
                            nodes
                        } else {
                            nodes
                        }
                    }
                    Op::Reduce(_, ReduceOp::ArgMin | ReduceOp::ArgMax, _) => nodes,
                }
            } else {
                nodes
            };
            already_seen.insert(node.id(), track_grad);
            if track_grad {
                nodes.push(node);
            }
            (track_grad, nodes)
        }
        let (_tg, mut nodes) = walk(self, vec![], &mut HashMap::new());
        nodes.reverse();
        nodes
    }

    pub fn backward(&self) -> Result<GradStore> {
        let sorted_nodes = self.sorted_nodes();
        let mut grads = GradStore::new();
        grads.insert(self, self.ones_like()?.contiguous()?);
        for node in sorted_nodes.iter() {
            if node.is_variable() {
                continue;
            }
            let grad = grads
                .remove(node)
                .expect("candle internal error - grad not populated");
            // https://github.com/huggingface/candle/issues/1241
            // Ideally, we would make these operations in place where possible to ensure that we
            // do not have to allocate too often. Here we just call `.detach` to avoid computing
            // the backprop graph of the backprop itself. This would be an issue for second order
            // derivatives but these are out of scope at the moment.
            let do_not_detach = CANDLE_GRAD_DO_NOT_DETACH.with(|b| *b);
            let grad = if do_not_detach { grad } else { grad.detach() };
            if let Some(op) = node.op() {
                match op {
                    Op::Binary(lhs, rhs, BinaryOp::Add) => {
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&grad)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.add(&grad)?;
                    }
                    Op::Binary(lhs, rhs, BinaryOp::Sub) => {
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&grad)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.sub(&grad)?;
                    }
                    Op::Binary(lhs, rhs, BinaryOp::Mul) => {
                        let lhs_grad = grad.mul(rhs)?;
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&lhs_grad)?;
                        let rhs_grad = grad.mul(lhs)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.add(&rhs_grad)?;
                    }
                    Op::Binary(lhs, rhs, BinaryOp::Div) => {
                        let lhs_grad = grad.div(rhs)?;
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&lhs_grad)?;
                        let rhs_grad = grad.mul(lhs)?.div(&rhs.sqr()?)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.sub(&rhs_grad)?;
                    }
                    Op::Binary(lhs, rhs, BinaryOp::Minimum)
                    | Op::Binary(lhs, rhs, BinaryOp::Maximum) => {
                        let mask_lhs = node.eq(lhs)?.to_dtype(grad.dtype())?;
                        let mask_rhs = node.eq(rhs)?.to_dtype(grad.dtype())?;

                        // If both masks are 1 one the same point, we want to scale the
                        // gradient by 0.5 rather than 1.
                        let lhs_grad = mask_lhs.mul(&grad)?.div(&(&mask_rhs + 1.)?)?;
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&lhs_grad)?;

                        let rhs_grad = mask_rhs.mul(&grad)?.div(&(&mask_lhs + 1.)?)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.add(&rhs_grad)?;
                    }
                    Op::WhereCond(pred, t, f) => {
                        let zeros = grad.zeros_like()?;
                        let t_sum_grad = grads.or_insert(t)?;
                        let t_grad = pred.where_cond(&grad, &zeros)?;
                        *t_sum_grad = t_sum_grad.add(&t_grad)?;
                        let f_sum_grad = grads.or_insert(f)?;
                        let f_grad = pred.where_cond(&zeros, &grad)?;
                        *f_sum_grad = f_sum_grad.add(&f_grad)?;
                    }
                    Op::Conv1D {
                        arg,
                        kernel,
                        padding,
                        stride,
                        dilation,
                    } => {
                        // The output height for conv_transpose1d is:
                        // (l_in - 1) * stride - 2 * padding + dilation * (k_size - 1) + out_padding + 1
                        let grad_l_in = grad.dim(2)?;
                        let k_size = kernel.dim(2)?;
                        let out_size =
                            (grad_l_in - 1) * stride + dilation * (k_size - 1) + 1 - 2 * padding;
                        let out_padding = arg.dim(2)? - out_size;
                        let grad_arg = grad.conv_transpose1d(
                            kernel,
                            *padding,
                            out_padding,
                            *stride,
                            *dilation,
                            /* groups */ 1,
                        )?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad_arg)?;

                        let grad_kernel = arg
                            .transpose(0, 1)?
                            .conv1d(&grad.transpose(0, 1)?, *padding, *dilation, *stride, 1)?
                            .transpose(0, 1)?;
                        let sum_grad = grads.or_insert(kernel)?;
                        let (_, _, k0) = kernel.dims3()?;
                        let (_, _, g_k0) = grad_kernel.dims3()?;
                        let grad_kernel = if g_k0 != k0 {
                            grad_kernel.narrow(2, 0, k0)?
                        } else {
                            grad_kernel
                        };
                        *sum_grad = sum_grad.add(&grad_kernel)?;
                    }
                    Op::Conv2D {
                        arg,
                        kernel,
                        padding,
                        stride,
                        dilation,
                    } => {
                        // The output height for conv_transpose2d is:
                        // (i_h - 1) * stride - 2 * padding + dilation * (k_h - 1) + out_padding + 1
                        let grad_h = grad.dim(2)?;
                        let k_h = kernel.dim(2)?;
                        let out_size =
                            (grad_h - 1) * stride + dilation * (k_h - 1) + 1 - 2 * padding;
                        let out_padding = arg.dim(2)? - out_size;
                        let grad_arg = grad.conv_transpose2d(
                            kernel,
                            *padding,
                            out_padding,
                            *stride,
                            *dilation,
                        )?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad_arg)?;

                        let grad_kernel = arg
                            .transpose(0, 1)?
                            .conv2d(&grad.transpose(0, 1)?, *padding, *dilation, *stride, 1)?
                            .transpose(0, 1)?;
                        let sum_grad = grads.or_insert(kernel)?;
                        let (_, _, k0, k1) = kernel.dims4()?;
                        let (_, _, g_k0, g_k1) = grad_kernel.dims4()?;
                        let grad_kernel = if g_k0 != k0 || g_k1 != k1 {
                            grad_kernel.narrow(2, 0, k0)?.narrow(3, 0, k1)?
                        } else {
                            grad_kernel
                        };
                        *sum_grad = sum_grad.add(&grad_kernel)?;
                    }
                    Op::ConvTranspose1D { .. } => Err(Error::BackwardNotSupported {
                        op: "conv-transpose1d",
                    })?,
                    Op::ConvTranspose2D {
                        arg,
                        kernel,
                        padding,
                        stride,
                        dilation,
                        output_padding: _output_padding,
                    } => {
                        let grad_arg = grad.conv2d(kernel, *padding, *stride, *dilation, 1)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad_arg)?;

                        let grad_kernel = grad
                            .transpose(0, 1)?
                            .conv2d(&arg.transpose(0, 1)?, *padding, *dilation, *stride, 1)?
                            .transpose(0, 1)?;
                        let sum_grad = grads.or_insert(kernel)?;
                        let (_, _, k0, k1) = kernel.dims4()?;
                        let (_, _, g_k0, g_k1) = grad_kernel.dims4()?;
                        let grad_kernel = if g_k0 != k0 || g_k1 != k1 {
                            grad_kernel.narrow(2, 0, k0)?.narrow(3, 0, k1)?
                        } else {
                            grad_kernel
                        };
                        *sum_grad = sum_grad.add(&grad_kernel)?;
                    }
                    Op::AvgPool2D {
                        arg,
                        kernel_size,
                        stride,
                    } => {
                        if kernel_size != stride {
                            crate::bail!("backward not supported for avgpool2d if ksize {kernel_size:?} != stride {stride:?}")
                        }
                        let (_n, _c, h, w) = arg.dims4()?;
                        let grad_arg = grad.upsample_nearest2d(h, w)?;
                        let grad_arg =
                            (grad_arg * (1f64 / (kernel_size.0 * kernel_size.1) as f64))?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad_arg)?;
                    }
                    Op::MaxPool2D {
                        arg,
                        kernel_size,
                        stride,
                    } => {
                        if kernel_size != stride {
                            crate::bail!("backward not supported for maxpool2d if ksize {kernel_size:?} != stride {stride:?}")
                        }
                        let (_n, _c, h, w) = arg.dims4()?;
                        // For computing the max-pool gradient, we compute a mask where a 1 means
                        // that the element is the maximum, then we apply this mask to the
                        // upsampled gradient (taking into account that multiple max may exist so
                        // we scale the gradient for this case).
                        let node_upsampled = node.upsample_nearest2d(h, w)?;
                        let mask = arg.eq(&node_upsampled)?.to_dtype(arg.dtype())?;
                        let avg = mask.avg_pool2d_with_stride(*kernel_size, *stride)?;
                        let grad_arg = ((grad * avg)?.upsample_nearest2d(h, w)? * mask)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad_arg)?;
                    }
                    Op::UpsampleNearest1D { arg, target_size } => {
                        let (_n, c, size) = arg.dims3()?;
                        if target_size % size != 0 {
                            crate::bail!("backward not supported for non integer upscaling factors")
                        }
                        let scale = target_size / size;

                        let kernel = Tensor::ones((c, 1, scale), arg.dtype(), arg.device())?;
                        let conv_sum = grad.conv1d(&kernel, 0, scale, 1, c)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = conv_sum;
                    }
                    Op::UpsampleNearest2D {
                        arg,
                        target_h,
                        target_w,
                    } => {
                        let (_n, c, h, w) = arg.dims4()?;
                        if target_h % h != 0 || target_w % w != 0 {
                            crate::bail!("backward not supported for non integer upscaling factors")
                        }
                        let scale_h = target_h / h;
                        let scale_w = target_w / w;

                        if scale_h != scale_w {
                            crate::bail!("backward not supported for non uniform upscaling factors")
                        };
                        let kernel =
                            Tensor::ones((c, 1, scale_h, scale_w), arg.dtype(), arg.device())?;
                        let conv_sum = grad.conv2d(&kernel, 0, scale_h, 1, c)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = conv_sum;
                    }
                    Op::UpsampleBilinear2D { .. } => {
                        crate::bail!("backward not supported for upsample_bilinear2d")
                    }
                    Op::SliceScatter0(lhs, rhs, start_rhs) => {
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        let rhs_grad = grad.narrow(0, *start_rhs, rhs.dim(0)?)?;
                        *rhs_sum_grad = rhs_sum_grad.add(&rhs_grad)?;

                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        let lhs_grad = grad.slice_scatter0(&rhs.zeros_like()?, *start_rhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&lhs_grad)?
                    }
                    Op::Gather(arg, indexes, dim) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.scatter_add(indexes, &grad, *dim)?;
                    }
                    Op::Scatter(init, indexes, src, dim) => {
                        let init_sum_grad = grads.or_insert(init)?;
                        *init_sum_grad = init_sum_grad.add(&grad)?;

                        let src_grad = grad.gather(indexes, *dim)?;
                        let src_sum_grad = grads.or_insert(src)?;
                        *src_sum_grad = src_sum_grad.add(&src_grad)?;
                    }
                    Op::ScatterAdd(init, indexes, src, dim) => {
                        let init_sum_grad = grads.or_insert(init)?;
                        let mask = init.ones_like()?;
                        let mask = mask.scatter(indexes, &mask.zeros_like()?, *dim)?;
                        *init_sum_grad = init_sum_grad.add(&grad.mul(&mask)?)?;

                        let src_grad = grad.gather(indexes, *dim)?;
                        let src_sum_grad = grads.or_insert(src)?;
                        *src_sum_grad = src_sum_grad.add(&src_grad)?;
                    }
                    Op::IndexAdd(init, indexes, src, dim) => {
                        let init_sum_grad = grads.or_insert(init)?;
                        *init_sum_grad = init_sum_grad.add(&grad)?;

                        let src_grad = grad.index_select(indexes, *dim)?;
                        let src_sum_grad = grads.or_insert(src)?;
                        *src_sum_grad = src_sum_grad.add(&src_grad)?;
                    }
                    Op::IndexSelect(arg, indexes, dim) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.index_add(indexes, &grad, *dim)?;
                    }
                    Op::Matmul(lhs, rhs) => {
                        // Skipping checks, the op went ok, we can skip
                        // the matmul size checks for now.

                        let lhs_grad = grad.matmul(&rhs.t()?)?;
                        let lhs_sum_grad = grads.or_insert(lhs)?;
                        *lhs_sum_grad = lhs_sum_grad.add(&lhs_grad)?;

                        let rhs_grad = lhs.t()?.matmul(&grad)?;
                        let rhs_sum_grad = grads.or_insert(rhs)?;
                        *rhs_sum_grad = rhs_sum_grad.add(&rhs_grad)?;
                    }
                    Op::Cat(args, dim) => {
                        let mut start_idx = 0;
                        for arg in args {
                            let len = arg.dims()[*dim];
                            let arg_grad = grad.narrow(*dim, start_idx, len)?;
                            let sum_grad = grads.or_insert(arg)?;
                            *sum_grad = sum_grad.add(&arg_grad)?;
                            start_idx += len;
                        }
                    }
                    Op::Broadcast(arg) => {
                        let arg_dims = arg.dims();
                        let node_dims = node.dims();
                        // The number of dims that have been inserted on the left.
                        let left_dims = node_dims.len() - arg_dims.len();
                        let mut sum_dims: Vec<usize> = (0..left_dims).collect();
                        for (dim, (node_dim, arg_dim)) in node_dims[left_dims..]
                            .iter()
                            .zip(arg_dims.iter())
                            .enumerate()
                        {
                            if node_dim != arg_dim {
                                sum_dims.push(dim + left_dims)
                            }
                        }

                        let mut arg_grad = grad.sum_keepdim(sum_dims.as_slice())?;
                        for _i in 0..left_dims {
                            arg_grad = arg_grad.squeeze(0)?
                        }
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad.broadcast_as(sum_grad.dims())?)?;
                    }
                    Op::Reduce(arg, ReduceOp::Sum, reduced_dims) => {
                        let grad = broadcast_back(arg, &grad, reduced_dims)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad)?;
                    }
                    Op::Reduce(arg, ReduceOp::Max, reduced_dims) => {
                        let node = broadcast_back(arg, node, reduced_dims)?;
                        let grad = broadcast_back(arg, &grad, reduced_dims)?;
                        let grad = node.eq(arg)?.to_dtype(grad.dtype())?.mul(&grad)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad.broadcast_as(sum_grad.dims())?)?;
                    }
                    Op::Reduce(arg, ReduceOp::Min, reduced_dims) => {
                        let node = broadcast_back(arg, node, reduced_dims)?;
                        let grad = broadcast_back(arg, &grad, reduced_dims)?;
                        let grad = node.eq(arg)?.to_dtype(grad.dtype())?.mul(&grad)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad.broadcast_as(sum_grad.dims())?)?;
                    }
                    Op::ToDType(arg) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad.to_dtype(arg.dtype())?)?
                    }
                    Op::Copy(arg) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&grad)?
                    }
                    Op::Affine { arg, mul, .. } => {
                        let arg_grad = grad.affine(*mul, 0.)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Unary(arg, UnaryOp::Log) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&(grad / arg)?)?
                    }
                    Op::Unary(arg, UnaryOp::Sin) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&(&grad * arg.cos())?)?
                    }
                    Op::Unary(arg, UnaryOp::Cos) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.sub(&(&grad * arg.sin())?)?
                    }
                    Op::Unary(arg, UnaryOp::Tanh) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let minus_dtanh = (node.sqr()? - 1.)?;
                        *sum_grad = sum_grad.sub(&(&grad * &minus_dtanh)?)?
                    }
                    Op::Unary(arg, UnaryOp::Abs) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let ones = arg.ones_like()?;
                        let abs_grad = arg.ge(&arg.zeros_like()?)?.where_cond(&ones, &ones.neg()?);
                        *sum_grad = sum_grad.add(&(&grad * abs_grad)?)?
                    }
                    Op::Unary(arg, UnaryOp::Exp) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&(&grad * *node)?)?
                    }
                    Op::Unary(arg, UnaryOp::Neg) => {
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.sub(&grad)?
                    }
                    Op::Unary(arg, UnaryOp::Recip) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let grad = (grad / arg.sqr()?)?;
                        *sum_grad = sum_grad.sub(&grad)?
                    }
                    &Op::Narrow(ref arg, dim, start_idx, len) => {
                        let arg_dims = arg.dims();
                        let left_pad = if start_idx == 0 {
                            None
                        } else {
                            let mut dims = arg_dims.to_vec();
                            dims[dim] = start_idx;
                            Some(Tensor::zeros(dims, grad.dtype(), grad.device())?)
                        };
                        let right_pad = arg_dims[dim] - start_idx - len;
                        let right_pad = if right_pad == 0 {
                            None
                        } else {
                            let mut dims = arg_dims.to_vec();
                            dims[dim] = right_pad;
                            Some(Tensor::zeros(dims, grad.dtype(), grad.device())?)
                        };
                        let arg_grad = match (left_pad, right_pad) {
                            (None, None) => grad,
                            (Some(l), None) => Tensor::cat(&[&l, &grad], dim)?,
                            (None, Some(r)) => Tensor::cat(&[&grad, &r], dim)?,
                            (Some(l), Some(r)) => Tensor::cat(&[&l, &grad, &r], dim)?,
                        };
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Unary(_, UnaryOp::Floor)
                    | Op::Unary(_, UnaryOp::Round)
                    | Op::Reduce(_, ReduceOp::ArgMin, _)
                    | Op::Reduce(_, ReduceOp::ArgMax, _)
                    | Op::Unary(_, UnaryOp::Sign)
                    | Op::Cmp(_, _) => {}
                    Op::Reshape(arg) => {
                        let arg_grad = grad.reshape(arg.dims())?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Unary(_, UnaryOp::Ceil) => Err(Error::BackwardNotSupported { op: "ceil" })?,
                    Op::Unary(arg, UnaryOp::Gelu) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let cube = arg.powf(3.)?;
                        let tanh = (0.0356774 * &cube + (0.797885 * arg)?)?.tanh()?;
                        let gelu_grad = (((0.5 * &tanh)?
                            + (0.0535161 * cube + (0.398942 * arg)?)? * (1. - tanh.powf(2.)?))?
                            + 0.5)?;
                        *sum_grad = sum_grad.add(&(&grad * gelu_grad)?)?
                    }
                    Op::Unary(arg, UnaryOp::Erf) => {
                        let sum_grad = grads.or_insert(arg)?;
                        // d/dx erf(x) = 2/sqrt(pi) * e^(-x^2)
                        let erf_grad =
                            (2. / std::f64::consts::PI.sqrt()) * (arg.sqr()?.neg()?).exp()?;
                        *sum_grad = sum_grad.add(&(&grad * erf_grad)?)?
                    }
                    Op::Unary(arg, UnaryOp::GeluErf) => {
                        let sum_grad = grads.or_insert(arg)?;
                        // d/dx gelu_erf(x) = 0.5 + 0.398942 e^(-x^2/2) x + 0.5 erf(x/sqrt(2))
                        let neg_half_square = (arg.sqr()?.neg()? / 2.)?;
                        let scaled_exp_arg = (0.398942 * neg_half_square.exp()? * arg)?;
                        let arg_scaled_sqrt = (arg / 2f64.sqrt())?;
                        let erf_scaled_sqrt = (0.5 * arg_scaled_sqrt.erf()?)?;
                        let gelu_erf_grad = (0.5 + scaled_exp_arg + erf_scaled_sqrt)?;
                        *sum_grad = sum_grad.add(&(&grad * gelu_erf_grad)?)?;
                    }
                    Op::Unary(arg, UnaryOp::Relu) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let relu_grad = arg.ge(&arg.zeros_like()?)?.to_dtype(arg.dtype())?;
                        *sum_grad = sum_grad.add(&(&grad * relu_grad)?)?
                    }
                    Op::Unary(arg, UnaryOp::Silu) => {
                        let sum_grad = grads.or_insert(arg)?;
                        // d/dx silu = sigmoid(x) * (1 + x * (1 - sigmoid(x))) = sigmoid(x) * (1 - node) + node
                        let sigmoid_arg = (arg.neg()?.exp()? + 1.)?.recip()?;
                        let silu_grad = &sigmoid_arg * (1. - *node) + *node;
                        *sum_grad = sum_grad.add(&(&grad * silu_grad)?)?
                    }
                    Op::Elu(arg, alpha) => {
                        // d/dx elu(x) = 1 for x > 0, alpha * e^x for x <= 0
                        let sum_grad = grads.or_insert(arg)?;
                        let zeros = arg.zeros_like()?;
                        let positive_mask = arg.gt(&zeros)?.to_dtype(arg.dtype())?;
                        let negative_mask = arg.le(&zeros)?.to_dtype(arg.dtype())?;
                        // node == alpha * (e^x - 1) for x <= 0, reuse it
                        let negative_exp_mask = (negative_mask * (*node + *alpha))?;
                        let combined_mask = (positive_mask + negative_exp_mask)?;
                        *sum_grad = sum_grad.add(&(grad * combined_mask)?)?
                    }
                    Op::Powf(arg, e) => {
                        let arg_grad = (&(grad * arg.powf(e - 1.)?)? * *e)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::CustomOp1(arg, c) => {
                        if let Some(arg_grad) = c.bwd(arg, node, &grad)? {
                            let sum_grad = grads.or_insert(arg)?;
                            *sum_grad = sum_grad.add(&arg_grad)?
                        }
                    }
                    Op::CustomOp2(arg1, arg2, c) => {
                        let (arg_grad1, arg_grad2) = c.bwd(arg1, arg2, node, &grad)?;
                        if let Some(arg_grad1) = arg_grad1 {
                            let sum_grad = grads.or_insert(arg1)?;
                            *sum_grad = sum_grad.add(&arg_grad1)?
                        }
                        if let Some(arg_grad2) = arg_grad2 {
                            let sum_grad = grads.or_insert(arg2)?;
                            *sum_grad = sum_grad.add(&arg_grad2)?
                        }
                    }
                    Op::CustomOp3(arg1, arg2, arg3, c) => {
                        let (arg_grad1, arg_grad2, arg_grad3) =
                            c.bwd(arg1, arg2, arg3, node, &grad)?;
                        if let Some(arg_grad1) = arg_grad1 {
                            let sum_grad = grads.or_insert(arg1)?;
                            *sum_grad = sum_grad.add(&arg_grad1)?
                        }
                        if let Some(arg_grad2) = arg_grad2 {
                            let sum_grad = grads.or_insert(arg2)?;
                            *sum_grad = sum_grad.add(&arg_grad2)?
                        }
                        if let Some(arg_grad3) = arg_grad3 {
                            let sum_grad = grads.or_insert(arg3)?;
                            *sum_grad = sum_grad.add(&arg_grad3)?
                        }
                    }
                    Op::Unary(arg, UnaryOp::Sqr) => {
                        let arg_grad = arg.mul(&grad)?.affine(2., 0.)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Unary(arg, UnaryOp::Sqrt) => {
                        let arg_grad = grad.div(node)?.affine(0.5, 0.)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::ToDevice(arg) => {
                        let sum_grad = grads.or_insert(arg)?;
                        let arg_grad = grad.to_device(sum_grad.device())?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Transpose(arg, dim1, dim2) => {
                        let arg_grad = grad.transpose(*dim1, *dim2)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                    Op::Permute(arg, dims) => {
                        let mut inv_dims = vec![0; dims.len()];
                        for (i, &dim_idx) in dims.iter().enumerate() {
                            inv_dims[dim_idx] = i
                        }
                        let arg_grad = grad.permute(inv_dims)?;
                        let sum_grad = grads.or_insert(arg)?;
                        *sum_grad = sum_grad.add(&arg_grad)?
                    }
                };
            }
        }
        Ok(grads)
    }
}

/// A store for gradients, associating a tensor id to the corresponding gradient tensor, used for back propagation.
#[derive(Debug)]
pub struct GradStore(HashMap<TensorId, Tensor>);

impl GradStore {
    /// Create a new gradient store
    fn new() -> Self {
        GradStore(HashMap::new())
    }

    /// Get the gradient tensor corresponding to the given tensor id
    pub fn get_id(&self, id: TensorId) -> Option<&Tensor> {
        self.0.get(&id)
    }

    /// Get the gradient tensor associated with the given tensor
    pub fn get(&self, tensor: &Tensor) -> Option<&Tensor> {
        self.0.get(&tensor.id())
    }

    /// Remove the gradient tensor associated with the given tensor, returning it if it exists
    pub fn remove(&mut self, tensor: &Tensor) -> Option<Tensor> {
        self.0.remove(&tensor.id())
    }

    /// Insert a gradient tensor associated with the given tensor, returning the previous gradient tensor if it existed
    pub fn insert(&mut self, tensor: &Tensor, grad: Tensor) -> Option<Tensor> {
        self.0.insert(tensor.id(), grad)
    }

    /// Insert a gradient tensor associated with the given tensor id, returning the previous gradient tensor if it existed
    pub fn insert_id(&mut self, id: TensorId, grad: Tensor) -> Option<Tensor> {
        self.0.insert(id, grad)
    }

    /// Get the gradient tensor associated with the given tensor, or, if it does not exist,
    /// insert a tensor of zeroes, with the same shape and type as the given tensors and return it
    fn or_insert(&mut self, tensor: &Tensor) -> Result<&mut Tensor> {
        use std::collections::hash_map::Entry;
        let grad = match self.0.entry(tensor.id()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let grad = tensor.zeros_like()?;
                entry.insert(grad)
            }
        };
        Ok(grad)
    }

    /// Get the tensor ids of the stored gradient tensors
    pub fn get_ids(&self) -> impl Iterator<Item = &TensorId> {
        self.0.keys()
    }
}
