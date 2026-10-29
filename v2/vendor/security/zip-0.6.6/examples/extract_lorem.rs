use std::io::prelude::*;

fn main() {
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <filename>", args[0]);
        return 1;
    }
    let fname = std::path::Path::new(&*args[1]);
    let zipfile = std::fs::File::open(fname).unwrap();

    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut file = match archive.by_name("test/lorem_ipsum.txt") {
        Ok(file) => file,
        Err(..) => {
            println!("File test/lorem_ipsum.txt not found");
            return 2;
        }
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    println!("{contents}");

    0
}
