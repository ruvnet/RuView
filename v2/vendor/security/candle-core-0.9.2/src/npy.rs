//! Numpy support for tensors.
//!
//! The spec for the npy format can be found in
//! [npy-format](https://docs.scipy.org/doc/numpy-1.14.2/neps/npy-format.html).
//! The functions from this module can be used to read tensors from npy/npz files
//! or write tensors to these files. A npy file contains a single tensor (unnamed)
//! whereas a npz file can contain multiple named tensors. npz files are also compressed.
//!
//! These two formats are easy to use in Python using the numpy library.
//!
//! ```python
//! import numpy as np
//! x = np.arange(10)
//!
//! # Write a npy file.
//! np.save("test.npy", x)
//!
//! # Read a value from the npy file.
//! x = np.load("test.npy")
//!
//! # Write multiple values to a npz file.
//! values = { "x": x, "x_plus_one": x + 1 }
//! np.savez("test.npz", **values)
//!
//! # Load multiple values from a npz file.
//! values = np.loadz("test.npz")
//! ```
use crate::{DType, Device, Error, Result, Shape, Tensor};
use byteorder::{LittleEndian, ReadBytesExt};
use half::{bf16, f16, slice::HalfFloatSliceExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

const NPY_MAGIC_STRING: &[u8] = b"\x93NUMPY";
const NPY_SUFFIX: &str = ".npy";

fn read_header<R: Read>(reader: &mut R) -> Result<String> {
    let mut magic_string = vec![0u8; NPY_MAGIC_STRING.len()];
    reader.read_exact(&mut magic_string)?;
    if magic_string != NPY_MAGIC_STRING {
        return Err(Error::Npy("magic string mismatch".to_string()));
    }
    let mut version = [0u8; 2];
    reader.read_exact(&mut version)?;
    let header_len_len = match version[0] {
        1 => 2,
        2 => 4,
        otherwise => return Err(Error::Npy(format!("unsupported version {otherwise}"))),
    };
    let mut header_len = vec![0u8; header_len_len];
    reader.read_exact(&mut header_len)?;
    let header_len = header_len
        .iter()
        .rev()
        .fold(0_usize, |acc, &v| 256 * acc + v as usize);
    let mut header = vec![0u8; header_len];
    reader.read_exact(&mut header)?;
    Ok(String::from_utf8_lossy(&header).to_string())
}

#[derive(Debug, PartialEq)]
struct Header {
    descr: DType,
    fortran_order: bool,
    shape: Vec<usize>,
}

impl Header {
    fn shape(&self) -> Shape {
        Shape::from(self.shape.as_slice())
    }

    fn to_string(&self) -> Result<String> {
        let fortran_order = if self.fortran_order { "True" } else { "False" };
        let mut shape = self
            .shape
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let descr = match self.descr {
            DType::BF16 => Err(Error::Npy("bf16 is not supported".into()))?,
            DType::F16 => "f2",
            DType::F32 => "f4",
            DType::F64 => "f8",
            DType::I16 => "i2",
            DType::I32 => "i4",
            DType::I64 => "i8",
            DType::U32 => "u4",
            DType::U8 => "u1",
            DType::F8E4M3 => Err(Error::Npy("f8e4m3 is not supported".into()))?,
            DType::F6E2M3 => Err(Error::Npy("f6e2m3 is not supported".into()))?,
            DType::F6E3M2 => Err(Error::Npy("f6e3m2 is not supported".into()))?,
            DType::F4 => Err(Error::Npy("f4 is not supported".into()))?,
            DType::F8E8M0 => Err(Error::Npy("f8e8m0 is not supported".into()))?,
        };
        if !shape.is_empty() {
            shape.push(',')
        }
        Ok(format!(
            "{{'descr': '<{descr}', 'fortran_order': {fortran_order}, 'shape': ({shape}), }}"
        ))
    }

    // Hacky parser for the npy header, a typical example would be:
    // {'descr': '<f8', 'fortran_order': False, 'shape': (128,), }
    fn parse(header: &str) -> Result<Header> {
        let header =
            header.trim_matches(|c: char| c == '{' || c == '}' || c == ',' || c.is_whitespace());

        let mut parts: Vec<String> = vec![];
        let mut start_index = 0usize;
        let mut cnt_parenthesis = 0i64;
        for (index, c) in header.char_indices() {
            match c {
                '(' => cnt_parenthesis += 1,
                ')' => cnt_parenthesis -= 1,
                ',' => {
                    if cnt_parenthesis == 0 {
                        parts.push(header[start_index..index].to_owned());
                        start_index = index + 1;
                    }
                }
                _ => {}
            }
        }
        parts.push(header[start_index..].to_owned());
        let mut part_map: HashMap<String, String> = HashMap::new();
        for part in parts.iter() {
            let part = part.trim();
            if !part.is_empty() {
                match part.split(':').collect::<Vec<_>>().as_slice() {
                    [key, value] => {
                        let key = key.trim_matches(|c: char| c == '\'' || c.is_whitespace());
                        let value = value.trim_matches(|c: char| c == '\'' || c.is_whitespace());
                        let _ = part_map.insert(key.to_owned(), value.to_owned());
                    }
                    _ => return Err(Error::Npy(format!("unable to parse header {header}"))),
                }
            }
        }
        let fortran_order = match part_map.get("fortran_order") {
            None => false,
            Some(fortran_order) => match fortran_order.as_ref() {
                "False" => false,
                "True" => true,
                _ => return Err(Error::Npy(format!("unknown fortran_order {fortran_order}"))),
            },
        };
        let descr = match part_map.get("descr") {
            None => return Err(Error::Npy("no descr in header".to_string())),
            Some(descr) => {
                if descr.is_empty() {
                    return Err(Error::Npy("empty descr".to_string()));
                }
                if descr.starts_with('>') {
                    return Err(Error::Npy(format!("little-endian descr {descr}")));
                }
                // the only supported types in tensor are:
                //     float64, float32, float16,
                //     complex64, complex128,
                //     int64, int32, int16, int8,
                //     uint8, and bool.
                match descr.trim_matches(|c: char| c == '=' || c == '<' || c == '|') {
                    "e" | "f2" => DType::F16,
                    "f" | "f4" => DType::F32,
                    "d" | "f8" => DType::F64,
                    "i" | "i4" => DType::I32,
                    "q" | "i8" => DType::I64,
                    "h" | "i2" => DType::I16,
                    // "b" | "i1" => DType::S8,
                    "B" | "u1" => DType::U8,
                    "I" | "u4" => DType::U32,
                    "?" | "b1" => DType::U8,
                    // "F" | "F4" => DType::C64,
                    // "D" | "F8" => DType::C128,
                    descr => return Err(Error::Npy(format!("unrecognized descr {descr}"))),
                }
            }
        };
        let shape = match part_map.get("shape") {
            None => return Err(Error::Npy("no shape in header".to_string())),
            Some(shape) => {
                let shape = shape.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
                if shape.is_empty() {
                    vec![]
                } else {
                    shape
                        .split(',')
                        .map(|v| v.trim().parse::<usize>())
                        .collect::<std::result::Result<Vec<_>, _>>()?
                }
            }
        };
        Ok(Header {
            descr,
            fortran_order,
            shape,
        })
    }
}

impl Tensor {
    // TODO: Add the possibility to read directly to a device?
    pub(crate) fn from_reader<R: std::io::Read>(
        shape: Shape,
        dtype: DType,
        reader: &mut R,
    ) -> Result<Self> {
        let elem_count = shape.elem_count();
        match dtype {
            DType::BF16 => {
                let mut data_t = vec![bf16::ZERO; elem_count];
                reader.read_u16_into::<LittleEndian>(data_t.reinterpret_cast_mut())?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::F16 => {
                let mut data_t = vec![f16::ZERO; elem_count];
                reader.read_u16_into::<LittleEndian>(data_t.reinterpret_cast_mut())?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::F32 => {
                let mut data_t = vec![0f32; elem_count];
                reader.read_f32_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::F64 => {
                let mut data_t = vec![0f64; elem_count];
                reader.read_f64_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::U8 => {
                let mut data_t = vec![0u8; elem_count];
                reader.read_exact(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::U32 => {
                let mut data_t = vec![0u32; elem_count];
                reader.read_u32_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::I16 => {
                let mut data_t = vec![0i16; elem_count];
                reader.read_i16_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::I32 => {
                let mut data_t = vec![0i32; elem_count];
                reader.read_i32_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::I64 => {
                let mut data_t = vec![0i64; elem_count];
                reader.read_i64_into::<LittleEndian>(&mut data_t)?;
                Tensor::from_vec(data_t, shape, &Device::Cpu)
            }
            DType::F8E4M3 => {
                let mut data_t = vec![0u8; elem_count];
                reader.read_exact(&mut data_t)?;
                let data_f8: Vec<float8::F8E4M3> =
                    data_t.into_iter().map(float8::F8E4M3::from_bits).collect();
                Tensor::from_vec(data_f8, shape, &Device::Cpu)
            }
            DType::F6E2M3 | DType::F6E3M2 | DType::F4 | DType::F8E8M0 => {
                Err(Error::UnsupportedDTypeForOp(dtype, "from_reader").bt())
            }
        }
    }

    /// Reads a npy file and return the stored multi-dimensional array as a tensor.
    pub fn read_npy<T: AsRef<Path>>(path: T) -> Result<Self> {
        let mut reader = File::open(path.as_ref())?;
        let header = read_header(&mut reader)?;
        let header = Header::parse(&header)?;
        if header.fortran_order {
            return Err(Error::Npy("fortran order not supported".to_string()));
        }
        Self::from_reader(header.shape(), header.descr, &mut reader)
    }

    /// Reads a npz file and returns the stored multi-dimensional arrays together with their names.
    pub fn read_npz<T: AsRef<Path>>(path: T) -> Result<Vec<(String, Self)>> {
        let zip_reader = BufReader::new(File::open(path.as_ref())?);
        let mut zip = zip::ZipArchive::new(zip_reader)?;
        let mut result = vec![];
        for i in 0..zip.len() {
            let mut reader = zip.by_index(i)?;
            let name = {
                let name = reader.name()?;
                name.strip_suffix(NPY_SUFFIX).unwrap_or(name.as_ref()).to_owned()
            };
            let header = read_header(&mut reader)?;
            let header = Header::parse(&header)?;
            if header.fortran_order {
                return Err(Error::Npy("fortran order not supported".to_string()));
            }
            let s = Self::from_reader(header.shape(), header.descr, &mut reader)?;
            result.push((name, s))
        }
        Ok(result)
    }

    /// Reads a npz file and returns the stored multi-dimensional arrays for some specified names.
    pub fn read_npz_by_name<T: AsRef<Path>>(path: T, names: &[&str]) -> Result<Vec<Self>> {
        let zip_reader = BufReader::new(File::open(path.as_ref())?);
        let mut zip = zip::ZipArchive::new(zip_reader)?;
        let mut result = vec![];
        for name in names.iter() {
            let mut reader = match zip.by_name(&format!("{name}{NPY_SUFFIX}")) {
                Ok(reader) => reader,
                Err(_) => Err(Error::Npy(format!(
                    "no array for {name} in {:?}",
                    path.as_ref()
                )))?,
            };
            let header = read_header(&mut reader)?;
            let header = Header::parse(&header)?;
            if header.fortran_order {
                return Err(Error::Npy("fortran order not supported".to_string()));
            }
            let s = Self::from_reader(header.shape(), header.descr, &mut reader)?;
            result.push(s)
        }
        Ok(result)
    }

    fn write<T: Write>(&self, f: &mut T) -> Result<()> {
        f.write_all(NPY_MAGIC_STRING)?;
        f.write_all(&[1u8, 0u8])?;
        let header = Header {
            descr: self.dtype(),
            fortran_order: false,
            shape: self.dims().to_vec(),
        };
        let mut header = header.to_string()?;
        let pad = 16 - (NPY_MAGIC_STRING.len() + 5 + header.len()) % 16;
        for _ in 0..pad % 16 {
            header.push(' ')
        }
        header.push('\n');
        f.write_all(&[(header.len() % 256) as u8, (header.len() / 256) as u8])?;
        f.write_all(header.as_bytes())?;
        self.write_bytes(f)
    }

    /// Writes a multi-dimensional array in the npy format.
    pub fn write_npy<T: AsRef<Path>>(&self, path: T) -> Result<()> {
        let mut f = File::create(path.as_ref())?;
        self.write(&mut f)
    }

    /// Writes multiple multi-dimensional arrays using the npz format.
    pub fn write_npz<S: AsRef<str>, T: AsRef<Tensor>, P: AsRef<Path>>(
        ts: &[(S, T)],
        path: P,
    ) -> Result<()> {
        let mut zip = zip::ZipWriter::new(File::create(path.as_ref())?);
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        for (name, tensor) in ts.iter() {
            zip.start_file(format!("{}.npy", name.as_ref()), options)?;
            tensor.as_ref().write(&mut zip)?
        }
        Ok(())
    }
}

/// Lazy tensor loader.
pub struct NpzTensors {
    index_per_name: HashMap<String, usize>,
    path: std::path::PathBuf,
    // We do not store a zip reader as it needs mutable access to extract data. Instead we
    // re-create a zip reader for each tensor.
}

impl NpzTensors {
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let zip_reader = BufReader::new(File::open(&path)?);
        let mut zip = zip::ZipArchive::new(zip_reader)?;
        let mut index_per_name = HashMap::new();
        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            let name = {
                let name = file.name()?;
                name.strip_suffix(NPY_SUFFIX).unwrap_or(name.as_ref()).to_owned()
            };
            index_per_name.insert(name, i);
        }
        Ok(Self {
            index_per_name,
            path,
        })
    }

    pub fn names(&self) -> Vec<&String> {
        self.index_per_name.keys().collect()
    }

    /// This only returns the shape and dtype for a named tensor. Compared to `get`, this avoids
    /// reading the whole tensor data.
    pub fn get_shape_and_dtype(&self, name: &str) -> Result<(Shape, DType)> {
        let index = match self.index_per_name.get(name) {
            None => crate::bail!("cannot find tensor {name}"),
            Some(index) => *index,
        };
        let zip_reader = BufReader::new(File::open(&self.path)?);
        let mut zip = zip::ZipArchive::new(zip_reader)?;
        let mut reader = zip.by_index(index)?;
        let header = read_header(&mut reader)?;
        let header = Header::parse(&header)?;
        Ok((header.shape(), header.descr))
    }

    pub fn get(&self, name: &str) -> Result<Option<Tensor>> {
        let index = match self.index_per_name.get(name) {
            None => return Ok(None),
            Some(index) => *index,
        };
        // We hope that the file has not changed since first reading it.
        let zip_reader = BufReader::new(File::open(&self.path)?);
        let mut zip = zip::ZipArchive::new(zip_reader)?;
        let mut reader = zip.by_index(index)?;
        let header = read_header(&mut reader)?;
        let header = Header::parse(&header)?;
        if header.fortran_order {
            return Err(Error::Npy("fortran order not supported".to_string()));
        }
        let tensor = Tensor::from_reader(header.shape(), header.descr, &mut reader)?;
        Ok(Some(tensor))
    }
}

#[cfg(test)]
mod tests {
    use super::Header;

    #[test]
    fn parse() {
        let h = "{'descr': '<f8', 'fortran_order': False, 'shape': (128,), }";
        assert_eq!(
            Header::parse(h).unwrap(),
            Header {
                descr: crate::DType::F64,
                fortran_order: false,
                shape: vec![128]
            }
        );
        let h = "{'descr': '<f4', 'fortran_order': True, 'shape': (256,1,128), }";
        let h = Header::parse(h).unwrap();
        assert_eq!(
            h,
            Header {
                descr: crate::DType::F32,
                fortran_order: true,
                shape: vec![256, 1, 128]
            }
        );
        assert_eq!(
            h.to_string().unwrap(),
            "{'descr': '<f4', 'fortran_order': True, 'shape': (256,1,128,), }"
        );

        let h = Header {
            descr: crate::DType::U32,
            fortran_order: false,
            shape: vec![],
        };
        assert_eq!(
            h.to_string().unwrap(),
            "{'descr': '<u4', 'fortran_order': False, 'shape': (), }"
        );
    }
}
