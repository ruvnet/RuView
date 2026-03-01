use anndists::dist::*;

use hnsw_rs::prelude::*;
use rand::Rng;
use std::iter;

fn generate(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghij";
    let mut rng = rand::rng();
    let one_char = || CHARSET[rng.random_range(0..CHARSET.len())] as char;
    iter::repeat_with(one_char).take(len).collect()
}

fn main() {
    let nb_elem = 500000; // number of possible words in the dictionary
    let max_nb_connection = 15;
    let nb_layer = 16.min((nb_elem as f32).ln().trunc() as usize);
    let ef_c = 200;
    let nb_words = 1000;
    let hns = Hnsw::<u16, DistLevenshtein>::new(
        max_nb_connection,
        nb_elem,
        nb_layer,
        ef_c,
        DistLevenshtein {},
    );
    let mut words = vec![];
    for _n in 1..nb_words {
        let tw = generate(5);
        words.push(tw);
    }
    words.push(String::from("abcdj"));
    //
    for (i, w) in words.iter().enumerate() {
        let vec: Vec<u16> = w.chars().map(|c| c as u16).collect();
        hns.insert((&vec, i));
    }
    // create a filter
    let mut filter: Vec<usize> = Vec::new();
    for i in 1..100 {
        filter.push(i);
    }
    //
    let ef_search: usize = 30;
    let tosearch: Vec<u16> = "abcde".chars().map(|c| c as u16).collect();
    //
    println!("========== search with filter ");
    let res = hns.search_filter(&tosearch, 10, ef_search, Some(&filter));
    for r in res {
        println!(
            "Word: {:?} Id: {:?} Distance: {:?}",
            words[r.d_id], r.d_id, r.distance
        );
    }
    println!("========== search without filter ");
    let res3 = hns.search(&tosearch, 10, ef_search);
    for r in res3 {
        println!(
            "Word: {:?} Id: {:?} Distance: {:?}",
            words[r.d_id], r.d_id, r.distance
        );
    }
}
