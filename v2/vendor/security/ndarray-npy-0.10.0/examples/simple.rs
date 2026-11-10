use ndarray::{array, Array2};
use ndarray_npy::{ReadNpyError, ReadNpyExt, WriteNpyError, WriteNpyExt};
use std::fs::File;
use std::io::BufWriter;

fn write_example() -> Result<(), WriteNpyError> {
    let arr: Array2<i32> = array![[1, 2, 3], [4, 5, 6]];
    let writer = BufWriter::new(File::create("array.npy")?);
    arr.write_npy(writer)?;
    Ok(())
}

fn read_example() -> Result<(), ReadNpyError> {
    let reader = File::open("array.npy")?;
    let arr = Array2::<i32>::read_npy(reader)?;
    println!("arr =\n{}", arr);
    Ok(())
}

fn main() {
    write_example().expect("failure writing array to file");
    read_example().expect("failure reading array from file");
}
