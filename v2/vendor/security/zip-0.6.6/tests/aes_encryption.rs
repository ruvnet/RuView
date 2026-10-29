#![cfg(feature = "aes-crypto")]

use std::io::{self, Read};
use zip::ZipArchive;

const SECRET_CONTENT: &str = "Lorem ipsum dolor sit amet";

const PASSWORD: &[u8] = b"helloworld";

#[test]
fn aes256_encrypted_uncompressed_file() {
    let mut v = Vec::new();
    v.extend_from_slice(include_bytes!("data/aes_archive.zip"));
    let mut archive = ZipArchive::new(io::Cursor::new(v)).expect("couldn't open test zip file");

    let mut file = archive
        .by_name_decrypt("secret_data_256_uncompressed", PASSWORD)
        .expect("couldn't find file in archive")
        .expect("invalid password");
    assert_eq!("secret_data_256_uncompressed", file.name());

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("couldn't read encrypted file");
    assert_eq!(SECRET_CONTENT, content);
}

#[test]
fn aes256_encrypted_file() {
    let mut v = Vec::new();
    v.extend_from_slice(include_bytes!("data/aes_archive.zip"));
    let mut archive = ZipArchive::new(io::Cursor::new(v)).expect("couldn't open test zip file");

    let mut file = archive
        .by_name_decrypt("secret_data_256", PASSWORD)
        .expect("couldn't find file in archive")
        .expect("invalid password");
    assert_eq!("secret_data_256", file.name());

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("couldn't read encrypted and compressed file");
    assert_eq!(SECRET_CONTENT, content);
}

#[test]
fn aes192_encrypted_file() {
    let mut v = Vec::new();
    v.extend_from_slice(include_bytes!("data/aes_archive.zip"));
    let mut archive = ZipArchive::new(io::Cursor::new(v)).expect("couldn't open test zip file");

    let mut file = archive
        .by_name_decrypt("secret_data_192", PASSWORD)
        .expect("couldn't find file in archive")
        .expect("invalid password");
    assert_eq!("secret_data_192", file.name());

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("couldn't read encrypted file");
    assert_eq!(SECRET_CONTENT, content);
}

#[test]
fn aes128_encrypted_file() {
    let mut v = Vec::new();
    v.extend_from_slice(include_bytes!("data/aes_archive.zip"));
    let mut archive = ZipArchive::new(io::Cursor::new(v)).expect("couldn't open test zip file");

    let mut file = archive
        .by_name_decrypt("secret_data_128", PASSWORD)
        .expect("couldn't find file in archive")
        .expect("invalid password");
    assert_eq!("secret_data_128", file.name());

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("couldn't read encrypted file");
    assert_eq!(SECRET_CONTENT, content);
}
