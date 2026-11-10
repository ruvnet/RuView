use memmap2::MmapMut;
use ndarray::ArrayViewMut3;
use ndarray_npy::{write_zeroed_npy, ViewMutNpyExt};
use std::fs::{File, OpenOptions};
use std::io;

fn print_file_sizes(file: &File) -> io::Result<()> {
    println!("Size of backing file:");
    println!(
        "  Apparent size:\t{:.2} GiB",
        file.metadata()?.len() as f64 / (1 << 30) as f64
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        println!(
            "  Actual disk usage:\t{:.2} kiB",
            file.metadata()?.blocks() as f64 / 2.
        );
    }
    #[cfg(not(unix))]
    {
        println!("Actual disk usage is unknown for this platform.");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "array.npy";

    // Create a (sparse if supported) file containing 64 GiB of zeroed data.
    println!("Creating the (sparse if supported) backing file...");
    let file = File::create(path)?;
    write_zeroed_npy::<f64, _>(&file, (1024, 2048, 4096))?;

    // Memory-map the file and create the mutable view.
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    print_file_sizes(&file)?;
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    let mut view_mut = ArrayViewMut3::<f64>::view_mut_npy(&mut mmap)?;

    // Modify an element near the middle of the data.
    println!("Modifying an element near the middle of the data...");
    view_mut[[500, 1000, 2000]] = 3.14;
    print_file_sizes(&file)?;

    Ok(())
}
