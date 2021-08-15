extern crate crc32fast;

use crc32fast::Hasher;

use std::fs;
use std::io::prelude::*;

pub fn zero_vec(size: usize) -> Vec<u8> {
    let mut vec: Vec<u8> = Vec::with_capacity(size);
    for i in 0..size {
        vec.push(0);
    }
    return vec;
}

pub fn crc32_file(filename: &str) -> u32 {
    let mut file = fs::File::open(filename).unwrap();
    let mut hasher = Hasher::new();

    // read the file in 1MiB pieces
    const BUF_SIZE: usize = 1024*1024;
    let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
    while let Ok(n) = file.read(&mut buf[..]) {
        if n != BUF_SIZE {
            let rest = &buf[0..n];
            hasher.update(rest);
            break;
        } else {
            hasher.update(&buf);
        }
    }
    return hasher.finalize();
}