mod benchmarks;

use criterion::criterion_main;

criterion_main!(
    benchmarks::affine::benches,
    benchmarks::binary::benches,
    benchmarks::broadcast::benches,
    benchmarks::copy::benches,
    benchmarks::conv_transpose2d::benches,
    benchmarks::matmul::benches,
    benchmarks::qmatmul::benches,
    benchmarks::random::benches,
    benchmarks::reduce::benches,
    benchmarks::unary::benches,
    benchmarks::where_cond::benches,
);
