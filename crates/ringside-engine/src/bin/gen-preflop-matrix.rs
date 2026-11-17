//! Regenerates `data/preflop_169.bin`; rebuild the crate afterwards to embed it.

use std::time::Instant;

use ringside_engine::PreflopMatrix;

fn main() {
    ringside_engine::eval::init();
    let start = Instant::now();
    let matrix = PreflopMatrix::compute();
    let elapsed = start.elapsed();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/preflop_169.bin");
    std::fs::write(path, matrix.to_bytes()).expect("write preflop matrix");
    println!(
        "computed 169x169 preflop matrix in {:.2} s on {} threads -> {path}",
        elapsed.as_secs_f64(),
        rayon::current_num_threads()
    );
}
