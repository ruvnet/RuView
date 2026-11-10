//! Dummy data types for experimental/future float formats
//!
//! These are placeholder types for experimental floating-point formats
//! that are defined in the safetensors spec but not yet fully implemented.

use crate::{DType, Error, Result, WithDType};

/// 6-bit float with 2 exponent bits and 3 mantissa bits (MX6 format)
/// This is a dummy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct F6E2M3;

/// 6-bit float with 3 exponent bits and 2 mantissa bits (MX6 format)
/// This is a dummy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct F6E3M2;

/// 4-bit float (MX4 format)
/// This is a dummy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct F4;

/// 8-bit float with 8 exponent bits and 0 mantissa bits
/// This is a dummy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct F8E8M0;

// Implement WithDType for dummy types
macro_rules! dummy_with_dtype {
    ($ty:ty, $dtype:ident) => {
        impl WithDType for $ty {
            const DTYPE: DType = DType::$dtype;

            fn from_f64(_v: f64) -> Self {
                panic!(
                    "{} is a dummy type and cannot be constructed",
                    stringify!($ty)
                )
            }

            fn to_f64(self) -> f64 {
                panic!(
                    "{} is a dummy type and cannot be converted",
                    stringify!($ty)
                )
            }

            fn to_scalar(self) -> crate::scalar::Scalar {
                panic!(
                    "{} is a dummy type and cannot be converted to scalar",
                    stringify!($ty)
                )
            }

            fn cpu_storage_ref(_data: &[Self]) -> crate::CpuStorageRef<'_> {
                panic!(
                    "{} is a dummy type and does not support storage",
                    stringify!($ty)
                )
            }

            fn to_cpu_storage_owned(_data: Vec<Self>) -> crate::CpuStorage {
                panic!(
                    "{} is a dummy type and does not support storage",
                    stringify!($ty)
                )
            }

            fn cpu_storage_data(_s: crate::CpuStorage) -> Result<Vec<Self>> {
                Err(Error::UnsupportedDTypeForOp(DType::$dtype, "cpu_storage_data").bt())
            }

            fn cpu_storage_as_slice(_s: &crate::CpuStorage) -> Result<&[Self]> {
                Err(Error::UnsupportedDTypeForOp(DType::$dtype, "cpu_storage_as_slice").bt())
            }
        }
    };
}

dummy_with_dtype!(F6E2M3, F6E2M3);
dummy_with_dtype!(F6E3M2, F6E3M2);
dummy_with_dtype!(F4, F4);
dummy_with_dtype!(F8E8M0, F8E8M0);

// Implement NumAssign traits for dummy types
macro_rules! dummy_num_assign {
    ($ty:ty) => {
        impl std::ops::AddAssign for $ty {
            fn add_assign(&mut self, _other: Self) {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::SubAssign for $ty {
            fn sub_assign(&mut self, _other: Self) {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::MulAssign for $ty {
            fn mul_assign(&mut self, _other: Self) {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::DivAssign for $ty {
            fn div_assign(&mut self, _other: Self) {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::RemAssign for $ty {
            fn rem_assign(&mut self, _other: Self) {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::Add for $ty {
            type Output = Self;
            fn add(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::Sub for $ty {
            type Output = Self;
            fn sub(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::Mul for $ty {
            type Output = Self;
            fn mul(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::Div for $ty {
            type Output = Self;
            fn div(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl std::ops::Rem for $ty {
            type Output = Self;
            fn rem(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl num_traits::Zero for $ty {
            fn zero() -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }

            fn is_zero(&self) -> bool {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl num_traits::One for $ty {
            fn one() -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }

        impl num_traits::Num for $ty {
            type FromStrRadixErr = std::num::ParseFloatError;

            fn from_str_radix(
                _str: &str,
                _radix: u32,
            ) -> std::result::Result<Self, Self::FromStrRadixErr> {
                panic!(
                    "{} is a dummy type and does not support parsing",
                    stringify!($ty)
                )
            }
        }

        impl crate::cpu::kernels::VecOps for $ty {
            fn min(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }

            fn max(self, _other: Self) -> Self {
                panic!(
                    "{} is a dummy type and does not support operations",
                    stringify!($ty)
                )
            }
        }
    };
}

dummy_num_assign!(F6E2M3);
dummy_num_assign!(F6E3M2);
dummy_num_assign!(F4);
dummy_num_assign!(F8E8M0);

// Display implementations
impl std::fmt::Display for F6E2M3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F6E2M3")
    }
}

impl std::fmt::Display for F6E3M2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F6E3M2")
    }
}

impl std::fmt::Display for F4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F4")
    }
}

impl std::fmt::Display for F8E8M0 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F8E8M0")
    }
}
