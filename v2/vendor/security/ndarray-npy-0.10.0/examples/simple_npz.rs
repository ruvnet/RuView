use ndarray::{array, Array1, Array2};
use ndarray_npy::{NpzReader, NpzWriter};
use std::fs::File;

fn write_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut npz = NpzWriter::new(File::create("arrays.npz")?);
    let a: Array2<i32> = array![[1, 2, 3], [4, 5, 6]];
    let b: Array1<i32> = array![7, 8, 9];
    npz.add_array("a", &a)?;
    npz.add_array("b", &b)?;
    npz.finish()?;
    Ok(())
}

fn read_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut npz = NpzReader::new(File::open("arrays.npz")?)?;
    let a: Array2<i32> = npz.by_name("a")?;
    let b: Array1<i32> = npz.by_name("b")?;
    println!("a =\n{}", a);
    println!("b =\n{}", b);
    Ok(())
}

fn main() {
    write_example().expect("failure writing arrays to file");
    read_example().expect("failure reading arrays from file");
}
