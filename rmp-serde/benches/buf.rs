#![feature(test)]

extern crate test;

use serde::{Deserialize, Serialize};

use test::Bencher;

#[bench]
fn bench_strings_1000(bencher: &mut Bencher) {
    bench_strings(bencher, 1000);
}

#[bench]
fn bench_strings_5000(bencher: &mut Bencher) {
    bench_strings(bencher, 5000);
}

#[bench]
fn bench_strings_10000(bencher: &mut Bencher) {
    bench_strings(bencher, 10000);
}

fn bench_strings(bencher: &mut Bencher, size: usize) {
    let vec: Vec<String> = ::std::iter::repeat("abcdefghijklmnopqrstuvwxyz".into())
        .take(size)
        .collect();

    let mut buf = Vec::new();
    vec.serialize(&mut messpack_serde::Serializer::new(&mut buf))
        .unwrap();

    bencher.iter(|| {
        <Vec<String>>::deserialize(&mut messpack_serde::Deserializer::new(&buf[..])).unwrap();
    });
}
