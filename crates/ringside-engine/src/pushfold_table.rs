//! Precomputed push/fold equilibria of [`multiway`](crate::multiway), shipped with
//! the app: chip EV, equal stacks, 2–9 players, for three ante structures
//! ([`Ante`]) on a fixed stack grid ([`stacks`]).
//!
//! A lookup only hits when the parameters equal the ones the table was solved with
//! ([`params`]); anything else has to be solved live. Frequencies are stored in
//! 1/255 steps and shove/call EVs in [`EV_STEP_BB`] steps, so a hit differs from a
//! fresh [`solve`](crate::multiway::solve) by at most half a step.
//!
//! # File format
//! One file per ante structure, regenerated with
//! `cargo run --release -p ringside-engine --bin gen-pushfold-tables`:
//!
//! ```text
//! header  magic "RSPF", format u32, ante u8, engine version (u8 length + UTF-8),
//!         entry count u32, body length u32, FNV-1a 64 of the body
//! body    deflate stream of the entries, each prefixed with its length (u32)
//! ```
//!
//! An entry holds the full solver parameters, the solution summary and every node of
//! the tree: seat, shover, callers, reach and fold EV, then the action frequencies of
//! all nodes as bytes and the shove/call EVs as i16 steps, each minus the same class
//! at the previous node, split into a low- and a high-byte plane: deflate packs those
//! far better than interleaved absolute values. All numbers are little-endian.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::multiway::{Approximation, Node, Params, Solution};
use crate::range::NUM_CLASSES;

const MAGIC: &[u8; 4] = b"RSPF";
pub const FORMAT_VERSION: u32 = 1;
/// Resolution of stored shove/call EVs.
pub const EV_STEP_BB: f64 = 0.01;
/// Ante per player of [`Ante::Classic`], in BB.
pub const CLASSIC_ANTE_BB: f64 = 0.125;
/// Ante of the big blind under [`Ante::BigBlind`], in BB.
pub const BB_ANTE_BB: f64 = 1.0;
pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = crate::multiway::MAX_PLAYERS;
const C: usize = NUM_CLASSES;
const STALE: &str = "run `cargo run --release -p ringside-engine --bin gen-pushfold-tables`";

/// Ante structure of a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ante {
    /// Blinds only.
    None,
    /// Every player pays [`CLASSIC_ANTE_BB`].
    Classic,
    /// The big blind alone pays [`BB_ANTE_BB`].
    BigBlind,
}

impl Ante {
    pub const ALL: [Ante; 3] = [Ante::None, Ante::Classic, Ante::BigBlind];

    pub fn key(self) -> &'static str {
        match self {
            Ante::None => "none",
            Ante::Classic => "classic",
            Ante::BigBlind => "big_blind",
        }
    }

    fn code(self) -> u8 {
        self as u8
    }

    /// The ante structure of `p` if it is one of the tabulated ones.
    pub fn of(p: &Params) -> Option<Ante> {
        Ante::ALL.into_iter().find(|a| {
            let (ante, bb_ante) = a.amounts();
            p.ante_bb == ante && p.bb_ante_bb == bb_ante
        })
    }

    fn amounts(self) -> (f64, f64) {
        match self {
            Ante::None => (0.0, 0.0),
            Ante::Classic => (CLASSIC_ANTE_BB, 0.0),
            Ante::BigBlind => (0.0, BB_ANTE_BB),
        }
    }
}

/// Stack depths in BB, finer where ranges change fastest. Short stacks start where
/// the big blind can still post its antes and blind.
pub fn stacks(ante: Ante) -> &'static [f64] {
    const GRID: [f64; 31] = [
        1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 10.0, 11.0,
        12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 22.0, 25.0,
    ];
    match ante {
        Ante::None => &GRID,
        Ante::Classic => &GRID[1..],
        Ante::BigBlind => &GRID[2..],
    }
}

/// The parameters a table entry was solved with.
pub fn params(ante: Ante, players: usize, stack_bb: f64) -> Params {
    let mut p = Params::equal(players, stack_bb);
    (p.ante_bb, p.bb_ante_bb) = ante.amounts();
    p
}

/// The precomputed solution for exactly these parameters.
pub fn lookup(p: &Params) -> Option<Solution> {
    let ante = Ante::of(p)?;
    let stack = *p.stacks_bb.first()?;
    let table = table(ante);
    let bytes = table.index.get(&(p.stacks_bb.len(), stack.to_bits()))?;
    let entry = decode_entry(bytes).unwrap_or_else(|e| panic!("push/fold table {}: {e}; {STALE}", ante.key()));
    (entry.params == *p).then_some(entry.solution)
}

/// Every node of the table at `players` × `stack_bb` with `ante`.
pub fn chart(ante: Ante, players: usize, stack_bb: f64) -> Option<Solution> {
    lookup(&params(ante, players, stack_bb))
}

// ---------------------------------------------------------------------------------
// Embedded tables.

struct Table {
    index: HashMap<(usize, u64), Vec<u8>>,
}

static NONE_BYTES: &[u8] = include_bytes!("../data/pushfold_none.bin");
static CLASSIC_BYTES: &[u8] = include_bytes!("../data/pushfold_classic.bin");
static BIG_BLIND_BYTES: &[u8] = include_bytes!("../data/pushfold_big_blind.bin");

/// Inflated on first use, one ante structure at a time.
static TABLES: [LazyLock<Table>; 3] = [
    LazyLock::new(|| load(Ante::None, NONE_BYTES)),
    LazyLock::new(|| load(Ante::Classic, CLASSIC_BYTES)),
    LazyLock::new(|| load(Ante::BigBlind, BIG_BLIND_BYTES)),
];

fn load(ante: Ante, bytes: &[u8]) -> Table {
    let entries = read_file(bytes, ante).unwrap_or_else(|e| panic!("push/fold table {} is invalid ({e}); {STALE}", ante.key()));
    let index = entries
        .into_iter()
        .map(|e| {
            let head = read_params(&mut Reader(&e)).expect("entry checked while reading the file");
            ((head.stacks_bb.len(), head.stacks_bb[0].to_bits()), e)
        })
        .collect();
    Table { index }
}

fn table(ante: Ante) -> &'static Table {
    &TABLES[ante.code() as usize]
}

/// Raw entries of an ante structure (for the generator and tests).
pub fn embedded_entries(ante: Ante) -> Vec<(Params, Solution)> {
    let mut out: Vec<(Params, Solution)> = table(ante)
        .index
        .values()
        .map(|b| {
            let e = decode_entry(b).expect("embedded entry");
            (e.params, e.solution)
        })
        .collect();
    out.sort_by(|a, b| {
        (a.0.stacks_bb.len(), a.0.stacks_bb[0]).partial_cmp(&(b.0.stacks_bb.len(), b.0.stacks_bb[0])).unwrap()
    });
    out
}

// ---------------------------------------------------------------------------------
// Encoding.

pub struct Entry {
    pub params: Params,
    pub solution: Solution,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// A whole table file from encoded entries (see [`encode_entry`]).
pub fn write_file(ante: Ante, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for e in entries {
        body.extend_from_slice(&(e.len() as u32).to_le_bytes());
        body.extend_from_slice(e);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.push(ante.code());
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    out.push(version.len() as u8);
    out.extend_from_slice(version);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&fnv1a(&body).to_le_bytes());
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(&body, 10));
    out
}

/// Header facts of a table file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub ante: Ante,
    pub engine_version: String,
    pub entries: usize,
    pub body_len: usize,
    pub checksum: u64,
}

pub fn file_info(bytes: &[u8]) -> Result<FileInfo, String> {
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return Err("bad magic".into());
    }
    let format = r.u32()?;
    if format != FORMAT_VERSION {
        return Err(format!("format {format}, expected {FORMAT_VERSION}"));
    }
    let ante = match r.u8()? {
        0 => Ante::None,
        1 => Ante::Classic,
        2 => Ante::BigBlind,
        x => return Err(format!("unknown ante code {x}")),
    };
    let len = r.u8()? as usize;
    let engine_version = String::from_utf8(r.take(len)?.to_vec()).map_err(|e| e.to_string())?;
    let entries = r.u32()? as usize;
    let body_len = r.u32()? as usize;
    let checksum = r.u64()?;
    Ok(FileInfo { ante, engine_version, entries, body_len, checksum })
}

fn read_file(bytes: &[u8], ante: Ante) -> Result<Vec<Vec<u8>>, String> {
    let info = file_info(bytes)?;
    if info.ante != ante {
        return Err(format!("holds {:?}, expected {ante:?}", info.ante));
    }
    let header = 4 + 4 + 1 + 1 + info.engine_version.len() + 4 + 4 + 8;
    let body = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[header..], info.body_len)
        .map_err(|e| format!("inflate: {e:?}"))?;
    if body.len() != info.body_len || fnv1a(&body) != info.checksum {
        return Err("checksum mismatch".into());
    }
    let mut r = Reader(&body);
    let mut out = Vec::with_capacity(info.entries);
    for _ in 0..info.entries {
        let len = r.u32()? as usize;
        let e = r.take(len)?.to_vec();
        decode_entry(&e)?;
        out.push(e);
    }
    if !r.0.is_empty() {
        return Err("trailing bytes".into());
    }
    Ok(out)
}

fn put_f32(out: &mut Vec<u8>, x: f64) {
    out.extend_from_slice(&(x as f32).to_le_bytes());
}

fn put_f64(out: &mut Vec<u8>, x: f64) {
    out.extend_from_slice(&x.to_le_bytes());
}

pub fn encode_entry(params: &Params, sol: &Solution) -> Result<Vec<u8>, String> {
    let n = params.stacks_bb.len();
    if params.stacks_bb.iter().any(|&s| s != params.stacks_bb[0]) || !params.payouts.is_empty() {
        return Err("tables hold equal stacks in chip EV only".into());
    }
    let mut out = Vec::new();
    out.push(n as u8);
    for x in [
        params.stacks_bb[0],
        params.small_blind_bb,
        params.ante_bb,
        params.bb_ante_bb,
        params.dead_bb,
        params.target_exploitability,
    ] {
        put_f64(&mut out, x);
    }
    out.extend_from_slice(&params.max_iterations.to_le_bytes());
    out.extend_from_slice(&params.multiway_samples.to_le_bytes());
    out.push(params.max_all_in as u8);
    out.push(params.card_removal as u8);

    out.extend_from_slice(&sol.iterations.to_le_bytes());
    out.push(sol.converged as u8);
    put_f64(&mut out, sol.exploitability);
    for &x in sol.ev.iter().chain(&sol.gain) {
        put_f32(&mut out, x);
    }
    let a = &sol.approximation;
    out.push(a.multiway_monte_carlo as u8);
    out.extend_from_slice(&a.multiway_samples.to_le_bytes());
    put_f32(&mut out, a.multiway_probability);
    put_f32(&mut out, a.capped_probability);
    out.push(a.pairwise_card_removal as u8);
    put_f32(&mut out, a.ev_sum_error);

    out.extend_from_slice(&(sol.nodes.len() as u16).to_le_bytes());
    for node in &sol.nodes {
        out.push(node.seat as u8);
        out.push(node.shover.map_or(u8::MAX, |s| s as u8));
        out.push(node.callers.len() as u8);
        out.extend(node.callers.iter().map(|&c| c as u8));
        put_f32(&mut out, node.reach);
        // In chip EV a fold loses exactly what was posted, whatever the cards.
        let fold = node.ev_fold[0];
        if node.ev_fold.iter().any(|&f| (f - fold).abs() > 1e-9) {
            return Err(format!("fold EV of seat {} depends on the class", node.seat));
        }
        put_f32(&mut out, fold);
    }
    for node in &sol.nodes {
        out.extend(node.act.iter().map(|&f| (f.clamp(0.0, 1.0) * 255.0).round() as u8));
    }
    let steps: Vec<i32> = sol.nodes.iter().flat_map(|node| &node.ev_act).map(|&ev| (ev / EV_STEP_BB).round() as i32).collect();
    // Half the i16 range, so that the differences fit too.
    if let Some(ev) = steps.iter().find(|q| q.abs() > i16::MAX as i32 / 2) {
        return Err(format!("EV {} BB out of range", *ev as f64 * EV_STEP_BB));
    }
    let evs: Vec<i16> = (0..steps.len()).map(|i| (steps[i] - if i < C { 0 } else { steps[i - C] }) as i16).collect();
    out.extend(evs.iter().map(|v| v.to_le_bytes()[0]));
    out.extend(evs.iter().map(|v| v.to_le_bytes()[1]));
    Ok(out)
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("truncated".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f64, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()) as f64)
    }
    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}

fn read_params(r: &mut Reader) -> Result<Params, String> {
    let n = r.u8()? as usize;
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
        return Err(format!("{n} players"));
    }
    let stack = r.f64()?;
    let mut p = Params::equal(n, stack);
    p.small_blind_bb = r.f64()?;
    p.ante_bb = r.f64()?;
    p.bb_ante_bb = r.f64()?;
    p.dead_bb = r.f64()?;
    p.target_exploitability = r.f64()?;
    p.max_iterations = r.u32()?;
    p.multiway_samples = r.u32()?;
    p.max_all_in = r.u8()? as usize;
    p.card_removal = r.u8()? != 0;
    Ok(p)
}

pub fn decode_entry(bytes: &[u8]) -> Result<Entry, String> {
    let mut r = Reader(bytes);
    let params = read_params(&mut r)?;
    let n = params.stacks_bb.len();
    let iterations = r.u32()?;
    let converged = r.u8()? != 0;
    let exploitability = r.f64()?;
    let ev = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
    let gain = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
    let approximation = Approximation {
        multiway_monte_carlo: r.u8()? != 0,
        multiway_samples: r.u32()?,
        multiway_probability: r.f32()?,
        capped_probability: r.f32()?,
        pairwise_card_removal: r.u8()? != 0,
        ev_sum_error: r.f32()?,
    };
    let count = r.u16()? as usize;
    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let seat = r.u8()? as usize;
        let shover = match r.u8()? {
            u8::MAX => None,
            s => Some(s as usize),
        };
        let k = r.u8()? as usize;
        let callers = r.take(k)?.iter().map(|&c| c as usize).collect();
        let reach = r.f32()?;
        let fold = r.f32()?;
        nodes.push(Node { seat, shover, callers, reach, act: Vec::new(), ev_act: Vec::new(), ev_fold: vec![fold; C] });
    }
    let act = r.take(count * C)?;
    let lo = r.take(count * C)?;
    let hi = r.take(count * C)?;
    let mut steps = [0i32; C];
    for (i, node) in nodes.iter_mut().enumerate() {
        let range = i * C..(i + 1) * C;
        node.act = act[range.clone()].iter().map(|&q| q as f64 / 255.0).collect();
        for (c, (&l, &h)) in lo[range.clone()].iter().zip(&hi[range]).enumerate() {
            steps[c] += i16::from_le_bytes([l, h]) as i32;
        }
        node.ev_act = steps.iter().map(|&q| q as f64 * EV_STEP_BB).collect();
    }
    if !r.0.is_empty() {
        return Err("trailing bytes in entry".into());
    }
    let solution = Solution { players: n, icm: false, nodes, ev, gain, exploitability, iterations, converged, approximation };
    Ok(Entry { params, solution })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiway::solve;

    /// Largest deviation of frequencies and shove/call EVs between two solutions with
    /// the same tree.
    fn deviation(a: &Solution, b: &Solution) -> (f64, f64) {
        assert_eq!(a.nodes.len(), b.nodes.len());
        let (mut freq, mut ev) = (0.0f64, 0.0f64);
        for (x, y) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!((x.seat, x.shover, &x.callers), (y.seat, y.shover, &y.callers));
            for c in 0..C {
                freq = freq.max((x.act[c] - y.act[c]).abs());
                ev = ev.max((x.ev_act[c] - y.ev_act[c]).abs()).max((x.ev_fold[c] - y.ev_fold[c]).abs());
            }
        }
        (freq, ev)
    }

    /// Quantisation bounds: half a frequency step, half an EV step plus f32 rounding.
    fn assert_close(table: &Solution, fresh: &Solution, what: &str) {
        let (freq, ev) = deviation(table, fresh);
        assert!(freq <= 0.5 / 255.0 + 1e-9, "{what}: frequency off by {freq}");
        assert!(ev <= 0.5 * EV_STEP_BB + 1e-4, "{what}: EV off by {ev}");
    }

    #[test]
    fn entry_round_trip_only_loses_quantisation() {
        let p = params(Ante::Classic, 3, 6.0);
        let sol = solve(&p).unwrap();
        let entry = decode_entry(&encode_entry(&p, &sol).unwrap()).unwrap();
        assert_eq!(entry.params, p);
        assert_close(&entry.solution, &sol, "round trip");
        assert_eq!(entry.solution.nodes.iter().map(|n| n.reach as f32).collect::<Vec<_>>(), sol.nodes.iter().map(|n| n.reach as f32).collect::<Vec<_>>());
        let file = write_file(Ante::Classic, &[encode_entry(&p, &sol).unwrap()]);
        assert_eq!(read_file(&file, Ante::Classic).unwrap().len(), 1);
        let mut broken = file.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(read_file(&broken, Ante::Classic).is_err());
        assert!(read_file(&file, Ante::None).is_err());
    }

    #[test]
    fn embedded_tables_cover_the_grid_and_nothing_else() {
        for ante in Ante::ALL {
            let bytes = [NONE_BYTES, CLASSIC_BYTES, BIG_BLIND_BYTES][ante.code() as usize];
            let info = file_info(bytes).unwrap();
            assert_eq!(info.ante, ante);
            let entries = embedded_entries(ante);
            assert_eq!(entries.len(), (MAX_PLAYERS - MIN_PLAYERS + 1) * stacks(ante).len());
            assert_eq!(info.entries, entries.len());
            for (p, sol) in &entries {
                assert_eq!(*p, params(ante, p.stacks_bb.len(), p.stacks_bb[0]));
                assert!(sol.converged, "{ante:?} {} players {} BB", sol.players, p.stacks_bb[0]);
            }
            for n in MIN_PLAYERS..=MAX_PLAYERS {
                for &stack in stacks(ante) {
                    assert!(chart(ante, n, stack).is_some(), "{ante:?} {n} players {stack} BB");
                }
            }
        }
        let mut p = params(Ante::Classic, 6, 10.0);
        assert!(lookup(&p).is_some());
        p.target_exploitability /= 2.0;
        assert!(lookup(&p).is_none());
        assert!(lookup(&params(Ante::Classic, 6, 10.25)).is_none());
        let mut p = params(Ante::Classic, 6, 10.0);
        p.stacks_bb[2] = 12.0;
        assert!(lookup(&p).is_none());
        assert!(lookup(&params(Ante::None, 6, 10.0).with_payouts(vec![0.5, 0.3, 0.2])).is_none());
    }

    #[test]
    fn small_tables_match_fresh_solves() {
        for ante in Ante::ALL {
            for (n, stack) in [(2, 3.0), (2, 12.0), (2, 25.0), (3, 8.0)] {
                let p = params(ante, n, stack);
                assert_close(&lookup(&p).unwrap(), &solve(&p).unwrap(), &format!("{ante:?} {n} players {stack} BB"));
            }
        }
    }

    /// The larger tables against fresh solves; run with
    /// `cargo test --release -p ringside-engine --lib pushfold_table -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sampled_tables_match_fresh_solves() {
        let samples = [
            (Ante::None, 9, 2.5),
            (Ante::None, 6, 13.0),
            (Ante::None, 4, 25.0),
            (Ante::Classic, 9, 18.0),
            (Ante::Classic, 7, 5.5),
            (Ante::Classic, 5, 10.0),
            (Ante::BigBlind, 9, 10.0),
            (Ante::BigBlind, 8, 3.0),
            (Ante::BigBlind, 6, 22.0),
        ];
        for (ante, n, stack) in samples {
            let p = params(ante, n, stack);
            let (table, fresh) = (lookup(&p).unwrap(), solve(&p).unwrap());
            let (freq, ev) = deviation(&table, &fresh);
            println!("{ante:?} {n} players {stack} BB: frequency off by {freq:.5}, EV off by {ev:.5} BB");
            assert_close(&table, &fresh, &format!("{ante:?} {n} players {stack} BB"));
        }
    }
}
