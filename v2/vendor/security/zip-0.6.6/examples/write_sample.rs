use std::io::prelude::*;
use zip::write::FileOptions;

fn main() {
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <filename>", args[0]);
        return 1;
    }

    let filename = &*args[1];
    match doit(filename) {
        Ok(_) => println!("File written to {filename}"),
        Err(e) => println!("Error: {e:?}"),
    }

    0
}

fn doit(filename: &str) -> zip::result::ZipResult<()> {
    let path = std::path::Path::new(filename);
    let file = std::fs::File::create(path).unwrap();

    let mut zip = zip::ZipWriter::new(file);

    zip.add_directory("test/", Default::default())?;

    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755);
    zip.start_file("test/☃.txt", options)?;
    zip.write_all(b"Hello, World!\n")?;

    zip.start_file("test/lorem_ipsum.txt", Default::default())?;
    zip.write_all(LOREM_IPSUM)?;

    zip.finish()?;
    Ok(())
}

const LOREM_IPSUM : &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. In tellus elit, tristique vitae mattis egestas, ultricies vitae risus. Quisque sit amet quam ut urna aliquet
molestie. Proin blandit ornare dui, a tempor nisl accumsan in. Praesent a consequat felis. Morbi metus diam, auctor in auctor vel, feugiat id odio. Curabitur ex ex,
dictum quis auctor quis, suscipit id lorem. Aliquam vestibulum dolor nec enim vehicula, porta tristique augue tincidunt. Vivamus ut gravida est. Sed pellentesque, dolor
vitae tristique consectetur, neque lectus pulvinar dui, sed feugiat purus diam id lectus. Class aptent taciti sociosqu ad litora torquent per conubia nostra, per
inceptos himenaeos. Maecenas feugiat velit in ex ultrices scelerisque id id neque.

Phasellus sed nisi in augue sodales pulvinar ut et leo. Pellentesque eget leo vitae massa bibendum sollicitudin. Curabitur erat lectus, congue quis auctor sed, aliquet
bibendum est. Ut porta ultricies turpis at maximus. Cras non lobortis justo. Duis rutrum magna sed velit facilisis, et sagittis metus laoreet. Pellentesque quam ligula,
dapibus vitae mauris quis, dapibus cursus leo. Sed sit amet condimentum eros. Nulla vestibulum enim sit amet lorem pharetra, eu fringilla nisl posuere. Sed tristique non
nibh at viverra. Vivamus sed accumsan lacus, nec pretium eros. Mauris elementum arcu eu risus fermentum, tempor ullamcorper neque aliquam. Sed tempor in erat eu
suscipit. In euismod in libero in facilisis. Donec sagittis, odio et fermentum dignissim, risus justo pretium nibh, eget vestibulum lectus metus vel lacus.

Quisque feugiat, magna ac feugiat ullamcorper, augue justo consequat felis, ut fermentum arcu lorem vitae ligula. Quisque iaculis tempor maximus. In quis eros ac tellus
aliquam placerat quis id tellus. Donec non gravida nulla. Morbi faucibus neque sed faucibus aliquam. Sed accumsan mattis nunc, non interdum justo. Cras vitae facilisis
leo. Fusce sollicitudin ultrices sagittis. Maecenas eget massa id lorem dignissim ultrices non et ligula. Pellentesque aliquam mi ac neque tempus ornare. Morbi non enim
vulputate quam ullamcorper finibus id non neque. Quisque malesuada commodo lorem, ut ornare velit iaculis rhoncus. Mauris vel maximus ex.

Morbi eleifend blandit diam, non vulputate ante iaculis in. Donec pellentesque augue id enim suscipit, eget suscipit lacus commodo. Ut vel ex vitae elit imperdiet
vulputate. Nunc eu mattis orci, ut pretium sem. Nam vitae purus mollis ante tempus malesuada a at magna. Integer mattis lectus non luctus lobortis. In a cursus quam,
eget faucibus sem.

Donec vitae condimentum nisi, non efficitur massa. Praesent sed mi in massa sollicitudin iaculis. Pellentesque a libero ultrices, sodales lacus eu, ornare dui. In
laoreet est nec dolor aliquam consectetur. Integer iaculis felis venenatis libero pulvinar, ut pretium odio interdum. Donec in nisi eu dolor varius vestibulum eget vel
nunc. Morbi a venenatis quam, in vehicula justo. Nam risus dui, auctor eu accumsan at, sagittis ac lectus. Mauris iaculis dignissim interdum. Cras cursus dapibus auctor.
Donec sagittis massa vitae tortor viverra vehicula. Mauris fringilla nunc eu lorem ultrices placerat. Maecenas posuere porta quam at semper. Praesent eu bibendum eros.
Nunc congue sollicitudin ante, sollicitudin lacinia magna cursus vitae.
";
