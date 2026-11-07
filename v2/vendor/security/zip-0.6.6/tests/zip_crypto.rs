// The following is a hexdump of a zip file containing the following
// ZipCrypto encrypted file:
// test.txt: 35 bytes, contents: `abcdefghijklmnopqrstuvwxyz123456789`, password: `test`
//
// 00000000  50 4b 03 04 14 00 01 00  00 00 54 bd b5 50 2f 20  |PK........T..P/ |
// 00000010  79 55 2f 00 00 00 23 00  00 00 08 00 00 00 74 65  |yU/...#.......te|
// 00000020  73 74 2e 74 78 74 ca 2d  1d 27 19 19 63 43 77 9a  |st.txt.-.'..cCw.|
// 00000030  71 76 c9 ec d1 6f d9 f5  22 67 b3 8f 52 b5 41 bc  |qv...o.."g..R.A.|
// 00000040  5c 36 f2 1d 84 c3 c0 28  3b fd e1 70 c2 cc 0c 11  |\6.....(;..p....|
// 00000050  0c c5 95 2f a4 50 4b 01  02 3f 00 14 00 01 00 00  |.../.PK..?......|
// 00000060  00 54 bd b5 50 2f 20 79  55 2f 00 00 00 23 00 00  |.T..P/ yU/...#..|
// 00000070  00 08 00 24 00 00 00 00  00 00 00 20 00 00 00 00  |...$....... ....|
// 00000080  00 00 00 74 65 73 74 2e  74 78 74 0a 00 20 00 00  |...test.txt.. ..|
// 00000090  00 00 00 01 00 18 00 31  b2 3b bf b8 2f d6 01 31  |.......1.;../..1|
// 000000a0  b2 3b bf b8 2f d6 01 a8  c4 45 bd b8 2f d6 01 50  |.;../....E../..P|
// 000000b0  4b 05 06 00 00 00 00 01  00 01 00 5a 00 00 00 55  |K..........Z...U|
// 000000c0  00 00 00 00 00                                    |.....|
// 000000c5

use std::io::Cursor;
use std::io::Read;

#[test]
fn encrypting_file() {
    use zip::unstable::write::FileOptionsExt;
    use std::io::{Read, Write};
    let mut buf = vec![0; 2048];
    let mut archive = zip::write::ZipWriter::new(std::io::Cursor::new(&mut buf));
    archive.start_file("name", zip::write::FileOptions::default().with_deprecated_encryption(b"password")).unwrap();
    archive.write_all(b"test").unwrap();
    archive.finish().unwrap();
    drop(archive);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&mut buf)).unwrap();
    let mut file = archive.by_index_decrypt(0, b"password").unwrap().unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, b"test");

}
#[test]
fn encrypted_file() {
    let zip_file_bytes = &mut Cursor::new(vec![
        0x50, 0x4b, 0x03, 0x04, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x54, 0xbd, 0xb5, 0x50, 0x2f,
        0x20, 0x79, 0x55, 0x2f, 0x00, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00,
        0x74, 0x65, 0x73, 0x74, 0x2e, 0x74, 0x78, 0x74, 0xca, 0x2d, 0x1d, 0x27, 0x19, 0x19, 0x63,
        0x43, 0x77, 0x9a, 0x71, 0x76, 0xc9, 0xec, 0xd1, 0x6f, 0xd9, 0xf5, 0x22, 0x67, 0xb3, 0x8f,
        0x52, 0xb5, 0x41, 0xbc, 0x5c, 0x36, 0xf2, 0x1d, 0x84, 0xc3, 0xc0, 0x28, 0x3b, 0xfd, 0xe1,
        0x70, 0xc2, 0xcc, 0x0c, 0x11, 0x0c, 0xc5, 0x95, 0x2f, 0xa4, 0x50, 0x4b, 0x01, 0x02, 0x3f,
        0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x54, 0xbd, 0xb5, 0x50, 0x2f, 0x20, 0x79, 0x55,
        0x2f, 0x00, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x08, 0x00, 0x24, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x74, 0x65, 0x73, 0x74,
        0x2e, 0x74, 0x78, 0x74, 0x0a, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x18,
        0x00, 0x31, 0xb2, 0x3b, 0xbf, 0xb8, 0x2f, 0xd6, 0x01, 0x31, 0xb2, 0x3b, 0xbf, 0xb8, 0x2f,
        0xd6, 0x01, 0xa8, 0xc4, 0x45, 0xbd, 0xb8, 0x2f, 0xd6, 0x01, 0x50, 0x4b, 0x05, 0x06, 0x00,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x5a, 0x00, 0x00, 0x00, 0x55, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let mut archive = zip::ZipArchive::new(zip_file_bytes).unwrap();

    assert_eq!(archive.len(), 1); //Only one file inside archive: `test.txt`

    {
        // No password
        let file = archive.by_index(0);
        match file {
            Err(zip::result::ZipError::UnsupportedArchive(
                zip::result::ZipError::PASSWORD_REQUIRED,
            )) => (),
            Err(_) => panic!(
                "Expected PasswordRequired error when opening encrypted file without password"
            ),
            Ok(_) => panic!("Error: Successfully opened encrypted file without password?!"),
        }
    }

    {
        // Wrong password
        let file = archive.by_index_decrypt(0, b"wrong password");
        match file {
            Ok(Err(zip::result::InvalidPassword)) => (),
            Err(_) => panic!(
                "Expected InvalidPassword error when opening encrypted file with wrong password"
            ),
            Ok(Ok(_)) => panic!("Error: Successfully opened encrypted file with wrong password?!"),
        }
    }

    {
        // Correct password, read contents
        let mut file = archive
            .by_index_decrypt(0, "test".as_bytes())
            .unwrap()
            .unwrap();
        let file_name = file.enclosed_name().unwrap();
        assert_eq!(file_name, std::path::PathBuf::from("test.txt"));

        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        assert_eq!(data, "abcdefghijklmnopqrstuvwxyz123456789".as_bytes());
    }
}
