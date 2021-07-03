use flate2::{bufread::DeflateEncoder, Compression};
use std::{
    env::var_os,
    fs::{read_dir, File},
    io::{copy, BufReader},
    path::Path,
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/");

    let out_dir = var_os("OUT_DIR").unwrap();

    // compress all files in data/ and save in the
    // OUT_DIR for later to be loaded with include_bytes!

    for input_file_dir_entry in read_dir("data").unwrap() {
        let input_file_dir_entry = input_file_dir_entry.unwrap();

        // open file
        let input_file = File::open(input_file_dir_entry.path()).unwrap();

        // compress
        let mut encoder = DeflateEncoder::new(BufReader::new(input_file), Compression::best());

        // write the compressed data to a file
        copy(
            &mut encoder,
            &mut File::create(Path::new(&out_dir).join(input_file_dir_entry.file_name())).unwrap(),
        )
        .unwrap();
    }
}
