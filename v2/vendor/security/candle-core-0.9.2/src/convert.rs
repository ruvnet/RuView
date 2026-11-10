//! Implement conversion traits for tensors
use crate::{DType, Device, Error, Tensor, WithDType};
use half::{bf16, f16, slice::HalfFloatSliceExt};
use std::convert::TryFrom;

impl<T: WithDType> TryFrom<&Tensor> for Vec<T> {
    type Error = Error;
    fn try_from(tensor: &Tensor) -> Result<Self, Self::Error> {
        tensor.to_vec1::<T>()
    }
}

impl<T: WithDType> TryFrom<&Tensor> for Vec<Vec<T>> {
    type Error = Error;
    fn try_from(tensor: &Tensor) -> Result<Self, Self::Error> {
        tensor.to_vec2::<T>()
    }
}

impl<T: WithDType> TryFrom<&Tensor> for Vec<Vec<Vec<T>>> {
    type Error = Error;
    fn try_from(tensor: &Tensor) -> Result<Self, Self::Error> {
        tensor.to_vec3::<T>()
    }
}

impl<T: WithDType> TryFrom<Tensor> for Vec<T> {
    type Error = Error;
    fn try_from(tensor: Tensor) -> Result<Self, Self::Error> {
        Vec::<T>::try_from(&tensor)
    }
}

impl<T: WithDType> TryFrom<Tensor> for Vec<Vec<T>> {
    type Error = Error;
    fn try_from(tensor: Tensor) -> Result<Self, Self::Error> {
        Vec::<Vec<T>>::try_from(&tensor)
    }
}

impl<T: WithDType> TryFrom<Tensor> for Vec<Vec<Vec<T>>> {
    type Error = Error;
    fn try_from(tensor: Tensor) -> Result<Self, Self::Error> {
        Vec::<Vec<Vec<T>>>::try_from(&tensor)
    }
}

impl<T: WithDType> TryFrom<&[T]> for Tensor {
    type Error = Error;
    fn try_from(v: &[T]) -> Result<Self, Self::Error> {
        Tensor::from_slice(v, v.len(), &Device::Cpu)
    }
}

impl<T: WithDType> TryFrom<Vec<T>> for Tensor {
    type Error = Error;
    fn try_from(v: Vec<T>) -> Result<Self, Self::Error> {
        let len = v.len();
        Tensor::from_vec(v, len, &Device::Cpu)
    }
}

macro_rules! from_tensor {
    ($typ:ident) => {
        impl TryFrom<&Tensor> for $typ {
            type Error = Error;

            fn try_from(tensor: &Tensor) -> Result<Self, Self::Error> {
                tensor.to_scalar::<$typ>()
            }
        }

        impl TryFrom<Tensor> for $typ {
            type Error = Error;

            fn try_from(tensor: Tensor) -> Result<Self, Self::Error> {
                $typ::try_from(&tensor)
            }
        }

        impl TryFrom<$typ> for Tensor {
            type Error = Error;

            fn try_from(v: $typ) -> Result<Self, Self::Error> {
                Tensor::new(v, &Device::Cpu)
            }
        }
    };
}

from_tensor!(f64);
from_tensor!(f32);
from_tensor!(f16);
from_tensor!(bf16);
from_tensor!(i64);
from_tensor!(i32);
from_tensor!(i16);
from_tensor!(u32);
from_tensor!(u8);

impl Tensor {
    pub fn write_bytes<W: std::io::Write>(&self, f: &mut W) -> crate::Result<()> {
        use byteorder::{LittleEndian, WriteBytesExt};

        let vs = self.flatten_all()?;
        match self.dtype() {
            DType::BF16 => {
                let vs = vs.to_vec1::<bf16>()?;
                for &v in vs.reinterpret_cast() {
                    f.write_u16::<LittleEndian>(v)?
                }
            }
            DType::F16 => {
                let vs = vs.to_vec1::<f16>()?;
                for &v in vs.reinterpret_cast() {
                    f.write_u16::<LittleEndian>(v)?
                }
            }
            DType::F32 => {
                // TODO: Avoid using a buffer when data is already on the CPU.
                for v in vs.to_vec1::<f32>()? {
                    f.write_f32::<LittleEndian>(v)?
                }
            }
            DType::F64 => {
                for v in vs.to_vec1::<f64>()? {
                    f.write_f64::<LittleEndian>(v)?
                }
            }
            DType::U32 => {
                for v in vs.to_vec1::<u32>()? {
                    f.write_u32::<LittleEndian>(v)?
                }
            }
            DType::I16 => {
                for v in vs.to_vec1::<i16>()? {
                    f.write_i16::<LittleEndian>(v)?
                }
            }
            DType::I32 => {
                for v in vs.to_vec1::<i32>()? {
                    f.write_i32::<LittleEndian>(v)?
                }
            }
            DType::I64 => {
                for v in vs.to_vec1::<i64>()? {
                    f.write_i64::<LittleEndian>(v)?
                }
            }
            DType::U8 => {
                let vs = vs.to_vec1::<u8>()?;
                f.write_all(&vs)?;
            }
            DType::F8E4M3 => {
                let vs = vs.to_vec1::<float8::F8E4M3>()?;
                for v in vs {
                    f.write_u8(v.to_bits())?
                }
            }
            DType::F6E2M3 | DType::F6E3M2 | DType::F4 | DType::F8E8M0 => {
                return Err(crate::Error::UnsupportedDTypeForOp(self.dtype(), "write_bytes").bt())
            }
        }
        Ok(())
    }
}
