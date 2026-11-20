use bencher::{benchmark_group, benchmark_main};

use std::io::{Cursor, Read, Seek, Write};

use bencher::Bencher;
use zip::{result::ZipResult, write::SimpleFileOptions, ZipArchive, ZipWriter};

fn generate_random_archive(
    num_entries: usize,
    entry_size: usize,
    options: SimpleFileOptions,
) -> ZipResult<(usize, ZipArchive<Cursor<Vec<u8>>>)> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);

    let mut bytes = vec![0u8; entry_size];
    for i in 0..num_entries {
        let name = format!("random{i}.dat");
        zip.start_file(name, options)?;
        getrandom::fill(&mut bytes).unwrap();
        zip.write_all(&bytes)?;
    }

    let buf = zip.finish()?.into_inner();
    let len = buf.len();

    Ok((len, ZipArchive::new(Cursor::new(buf))?))
}

fn perform_merge<R: Read + Seek, W: Write + Seek>(
    src: ZipArchive<R>,
    mut target: ZipWriter<W>,
) -> ZipResult<ZipWriter<W>> {
    target.merge_archive(src)?;
    Ok(target)
}

fn perform_raw_copy_file<R: Read + Seek, W: Write + Seek>(
    mut src: ZipArchive<R>,
    mut target: ZipWriter<W>,
) -> ZipResult<ZipWriter<W>> {
    for i in 0..src.len() {
        let entry = src.by_index(i)?;
        target.raw_copy_file(entry)?;
    }
    Ok(target)
}

const NUM_ENTRIES: usize = 100;
const ENTRY_SIZE: usize = 1024;

fn merge_archive_stored(bench: &mut Bencher) {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let (len, src) = generate_random_archive(NUM_ENTRIES, ENTRY_SIZE, options).unwrap();

    bench.bytes = len as u64;

    bench.iter(|| {
        let buf = Cursor::new(Vec::new());
        let zip = ZipWriter::new(buf);
        let zip = perform_merge(src.clone(), zip).unwrap();
        let buf = zip.finish().unwrap().into_inner();
        assert_eq!(buf.len(), len);
    });
}

#[cfg(feature = "_deflate-any")]
fn merge_archive_compressed(bench: &mut Bencher) {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let (len, src) = generate_random_archive(NUM_ENTRIES, ENTRY_SIZE, options).unwrap();

    bench.bytes = len as u64;

    bench.iter(|| {
        let buf = Cursor::new(Vec::new());
        let zip = ZipWriter::new(buf);
        let zip = perform_merge(src.clone(), zip).unwrap();
        let buf = zip.finish().unwrap().into_inner();
        assert_eq!(buf.len(), len);
    });
}

fn merge_archive_raw_copy_file_stored(bench: &mut Bencher) {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let (len, src) = generate_random_archive(NUM_ENTRIES, ENTRY_SIZE, options).unwrap();

    bench.bytes = len as u64;

    bench.iter(|| {
        let buf = Cursor::new(Vec::new());
        let zip = ZipWriter::new(buf);
        let zip = perform_raw_copy_file(src.clone(), zip).unwrap();
        let buf = zip.finish().unwrap().into_inner();
        assert_eq!(buf.len(), len);
    });
}

#[cfg(feature = "_deflate-any")]
fn merge_archive_raw_copy_file_compressed(bench: &mut Bencher) {
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let (len, src) = generate_random_archive(NUM_ENTRIES, ENTRY_SIZE, options).unwrap();

    bench.bytes = len as u64;

    bench.iter(|| {
        let buf = Cursor::new(Vec::new());
        let zip = ZipWriter::new(buf);
        let zip = perform_raw_copy_file(src.clone(), zip).unwrap();
        let buf = zip.finish().unwrap().into_inner();
        assert_eq!(buf.len(), len);
    });
}

#[cfg(feature = "_deflate-any")]
benchmark_group!(
    benches,
    merge_archive_stored,
    merge_archive_compressed,
    merge_archive_raw_copy_file_stored,
    merge_archive_raw_copy_file_compressed,
);

#[cfg(not(feature = "_deflate-any"))]
benchmark_group!(
    benches,
    merge_archive_stored,
    merge_archive_raw_copy_file_stored,
);

benchmark_main!(benches);
