use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::Result;

pub fn load_imatrix<P: AsRef<Path>>(fname: P) -> Result<HashMap<String, Vec<f32>>> {
    let mut all_data = HashMap::new();

    let mut file = File::open(&fname).map_err(|e| {
        crate::Error::msg(format!(
            "Failed to open {}: {}",
            fname.as_ref().display(),
            e
        ))
    })?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| {
        crate::Error::msg(format!(
            "Failed to read file {}: {}",
            fname.as_ref().display(),
            e
        ))
    })?;

    let mut cursor = Cursor::new(buffer);

    let n_entries = cursor
        .read_i32::<LittleEndian>()
        .map_err(|e| crate::Error::msg(format!("Failed to read number of entries: {e}")))?
        as usize;

    if n_entries < 1 {
        crate::bail!("No data in file {}", fname.as_ref().display());
    }

    for i in 0..n_entries {
        // Read length of the name
        let len = cursor.read_i32::<LittleEndian>().map_err(|e| {
            crate::Error::msg(format!(
                "Failed to read name length for entry {}: {}",
                i + 1,
                e
            ))
        })? as usize;

        // Read the name
        let mut name_buf = vec![0u8; len];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            crate::Error::msg(format!("Failed to read name for entry {}: {}", i + 1, e))
        })?;
        let name = String::from_utf8(name_buf).map_err(|e| {
            crate::Error::msg(format!("Invalid UTF-8 name for entry {}: {}", i + 1, e))
        })?;

        // Read ncall and nval
        let ncall = cursor.read_i32::<LittleEndian>().map_err(|e| {
            crate::Error::msg(format!("Failed to read ncall for entry {}: {}", i + 1, e))
        })? as usize;

        let nval = cursor.read_i32::<LittleEndian>().map_err(|e| {
            crate::Error::msg(format!("Failed to read nval for entry {}: {}", i + 1, e))
        })? as usize;

        if nval < 1 {
            crate::bail!("Invalid nval for entry {}: {}", i + 1, nval);
        }

        let mut data = Vec::with_capacity(nval);
        for _ in 0..nval {
            let v = cursor.read_f32::<LittleEndian>().unwrap();
            if ncall == 0 {
                data.push(v);
            } else {
                data.push(v / ncall as f32);
            }
        }
        all_data.insert(name, data);
    }

    Ok(all_data)
}
