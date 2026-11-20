use bencher::{benchmark_group, benchmark_main};

use std::io::{Cursor, Read, Write};

use bencher::Bencher;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

fn generate_random_archive(size: usize) -> Vec<u8> {
    let data = Vec::new();
    let mut writer = ZipWriter::new(Cursor::new(data));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    writer.start_file("random.dat", options).unwrap();
    let mut bytes = vec![0u8; size];
    getrandom::fill(&mut bytes).unwrap();
    writer.write_all(&bytes).unwrap();

    writer.finish().unwrap().into_inner()
}

fn read_entry(bench: &mut Bencher) {
    let size = 1024 * 1024;
    let bytes = generate_random_archive(size);
    let mut archive = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();

    bench.iter(|| {
        let mut file = archive.by_name("random.dat").unwrap();
        let mut buf = [0u8; 1024];
        loop {
            let n = file.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
        }
    });

    bench.bytes = size as u64;
}

benchmark_group!(benches, read_entry);
benchmark_main!(benches);
