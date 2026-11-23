use std::fs;
use std::io::BufReader;

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
    let file = fs::File::open(fname).unwrap();
    let reader = BufReader::new(file);

    let mut archive = zip::ZipArchive::new(reader).unwrap();

    for i in 0..archive.len() {
        let file = archive.by_index(i).unwrap();
        let outpath = match file.enclosed_name() {
            Some(path) => path,
            None => {
                println!("Entry {} has a suspicious path", file.name());
                continue;
            }
        };

        {
            let comment = file.comment();
            if !comment.is_empty() {
                println!("Entry {i} comment: {comment}");
            }
        }

        if (*file.name()).ends_with('/') {
            println!(
                "Entry {} is a directory with name \"{}\"",
                i,
                outpath.display()
            );
        } else {
            println!(
                "Entry {} is a file with name \"{}\" ({} bytes)",
                i,
                outpath.display(),
                file.size()
            );
        }
    }

    0
}
