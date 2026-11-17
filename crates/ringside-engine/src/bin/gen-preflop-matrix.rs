//! Regenerates `data/preflop_169.bin` and `data/preflop_169_ties.bin`; rebuild the
//! crate afterwards to embed them.

use std::time::Instant;

use ringside_engine::PreflopMatrix;

fn main() {
    ringside_engine::eval::init();
    let start = Instant::now();
    let (matrix, ties) = PreflopMatrix::compute_with_ties();
    let elapsed = start.elapsed();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/data");
    std::fs::write(format!("{dir}/preflop_169.bin"), matrix.to_bytes()).expect("write preflop matrix");
    std::fs::write(format!("{dir}/preflop_169_ties.bin"), ties.to_bytes()).expect("write tie matrix");
    println!(
        "computed 169x169 preflop and tie matrices in {:.2} s on {} threads -> {dir}",
        elapsed.as_secs_f64(),
        rayon::current_num_threads()
    );
}
