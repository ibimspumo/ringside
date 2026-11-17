//! Precomputed solutions of the [`preflop`](crate::preflop) model, shipped with the
//! app: chip EV, equal stacks, big-blind ante, 6, 8 and 9 players on a fixed stack
//! grid ([`STACKS`]).
//!
//! A lookup only hits when the parameters equal the ones the table was solved with
//! ([`params`]). Frequencies are stored in 1/255 steps and action EVs in
//! [`EV_STEP_BB`] steps, so a hit differs from a fresh [`solve`](crate::preflop::solve)
//! by at most half a step.
//!
//! # File format
//! One file per player count, regenerated with
//! `cargo run --release -p ringside-engine --bin gen-preflop-tables`:
//!
//! ```text
//! header  magic "RSPR", format u32, players u8, engine version (u8 length + UTF-8),
//!         entry count u32, body length u32, FNV-1a 64 of the body
//! body    deflate stream of the entries, each prefixed with its length (u32)
//! ```
//!
//! An entry holds the full solver parameters and the solution summary, then per node
//! its reach, the frequency of every action and class as bytes and the action EVs as
//! i16 steps (each minus the same class one row earlier), split into a low- and a
//! high-byte plane. The tree itself is not stored: it follows from the parameters and
//! is rebuilt on decoding. All numbers are little-endian.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::preflop::{Action, Kind, Node, Params, Realized, Solution, Step, skeleton};
use crate::range::NUM_CLASSES;

const MAGIC: &[u8; 4] = b"RSPR";
pub const FORMAT_VERSION: u32 = 1;
/// Resolution of stored action EVs.
pub const EV_STEP_BB: f64 = 0.01;
/// Ante of the big blind, in BB.
pub const BB_ANTE_BB: f64 = 1.0;
/// NashConv target of the tables, BB per hand summed over all players.
pub const TARGET_EXPLOITABILITY: f64 = 0.001;
pub const MAX_ITERATIONS: u32 = 4000;
pub const PLAYERS: [usize; 3] = [6, 8, 9];
/// Stack depths in BB.
pub const STACKS: [f64; 8] = [15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0];
const C: usize = NUM_CLASSES;
const STALE: &str = "run `cargo run --release -p ringside-engine --bin gen-preflop-tables`";

/// The parameters a table entry was solved with.
pub fn params(players: usize, stack_bb: f64) -> Params {
    let mut p = Params::new(players, stack_bb);
    p.bb_ante_bb = BB_ANTE_BB;
    p.target_exploitability = TARGET_EXPLOITABILITY;
    p.max_iterations = MAX_ITERATIONS;
    p
}

/// The precomputed solution for exactly these parameters.
pub fn lookup(p: &Params) -> Option<Solution> {
    let table = table(p.players)?;
    let bytes = table.index.get(&p.stack_bb.to_bits())?;
    let entry = decode_entry(bytes).unwrap_or_else(|e| panic!("preflop table {}: {e}; {STALE}", p.players));
    (entry.params == *p).then_some(entry.solution)
}

/// The table solution at `players` × `stack_bb`.
pub fn chart(players: usize, stack_bb: f64) -> Option<Solution> {
    lookup(&params(players, stack_bb))
}

// ---------------------------------------------------------------------------------
// Embedded tables.

struct Table {
    index: HashMap<u64, Vec<u8>>,
}

static SIX_BYTES: &[u8] = include_bytes!("../data/preflop_6max.bin");
static EIGHT_BYTES: &[u8] = include_bytes!("../data/preflop_8max.bin");
static NINE_BYTES: &[u8] = include_bytes!("../data/preflop_9max.bin");

/// Inflated on first use, one player count at a time.
static TABLES: [LazyLock<Table>; 3] = [
    LazyLock::new(|| load(6, SIX_BYTES)),
    LazyLock::new(|| load(8, EIGHT_BYTES)),
    LazyLock::new(|| load(9, NINE_BYTES)),
];

fn load(players: usize, bytes: &[u8]) -> Table {
    let entries = read_file(bytes, players).unwrap_or_else(|e| panic!("preflop table {players} is invalid ({e}); {STALE}"));
    let index = entries
        .into_iter()
        .map(|e| {
            let head = read_params(&mut Reader(&e)).expect("entry checked while reading the file");
            (head.stack_bb.to_bits(), e)
        })
        .collect();
    Table { index }
}

fn table(players: usize) -> Option<&'static Table> {
    PLAYERS.iter().position(|&n| n == players).map(|i| &*TABLES[i])
}

/// The embedded file of a player count (for the generator and tests).
pub fn embedded_file(players: usize) -> Option<&'static [u8]> {
    PLAYERS.iter().position(|&n| n == players).map(|i| [SIX_BYTES, EIGHT_BYTES, NINE_BYTES][i])
}

/// Every entry of a player count, by stack.
pub fn embedded_entries(players: usize) -> Vec<(Params, Solution)> {
    let Some(table) = table(players) else { return Vec::new() };
    let mut out: Vec<(Params, Solution)> = table
        .index
        .values()
        .map(|b| {
            let e = decode_entry(b).expect("embedded entry");
            (e.params, e.solution)
        })
        .collect();
    out.sort_by(|a, b| a.0.stack_bb.total_cmp(&b.0.stack_bb));
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
pub fn write_file(players: usize, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for e in entries {
        body.extend_from_slice(&(e.len() as u32).to_le_bytes());
        body.extend_from_slice(e);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.push(players as u8);
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
    pub players: usize,
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
    let players = r.u8()? as usize;
    let len = r.u8()? as usize;
    let engine_version = String::from_utf8(r.take(len)?.to_vec()).map_err(|e| e.to_string())?;
    let entries = r.u32()? as usize;
    let body_len = r.u32()? as usize;
    let checksum = r.u64()?;
    Ok(FileInfo { players, engine_version, entries, body_len, checksum })
}

fn read_file(bytes: &[u8], players: usize) -> Result<Vec<Vec<u8>>, String> {
    let info = file_info(bytes)?;
    if info.players != players {
        return Err(format!("holds {} players, expected {players}", info.players));
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

fn put_steps(out: &mut Vec<u8>, line: &[Step]) {
    out.push(line.len() as u8);
    for s in line {
        out.push(s.seat as u8);
        out.push(kind_code(s.action.kind));
        put_f32(out, s.action.to_bb);
    }
}

const KINDS: [Kind; 6] = [Kind::Fold, Kind::Check, Kind::Call, Kind::Limp, Kind::Raise, Kind::AllIn];

fn kind_code(k: Kind) -> u8 {
    KINDS.iter().position(|&x| x == k).expect("every kind is listed") as u8
}

pub fn encode_entry(params: &Params, sol: &Solution) -> Result<Vec<u8>, String> {
    let n = params.players;
    let shape = skeleton(params).map_err(|e| e.to_string())?;
    if shape.len() != sol.nodes.len() {
        return Err(format!("{} nodes, the tree has {}", sol.nodes.len(), shape.len()));
    }
    let mut out = Vec::new();
    out.push(n as u8);
    for x in params.numbers() {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out.push(params.limps as u8);
    out.push(params.card_removal as u8);
    out.extend_from_slice(&params.max_iterations.to_le_bytes());
    out.extend_from_slice(&params.multiway_samples.to_le_bytes());

    out.extend_from_slice(&sol.iterations.to_le_bytes());
    out.push(sol.converged as u8);
    out.extend_from_slice(&sol.exploitability.to_le_bytes());
    for &x in sol.ev.iter().chain(&sol.gain) {
        put_f32(&mut out, x);
    }
    put_f32(&mut out, sol.ev_sum_error);
    out.push(sol.realized.len() as u8);
    for r in &sol.realized {
        put_steps(&mut out, &r.line);
        out.extend(r.seats.iter().map(|&s| s as u8));
        for x in [r.reach, r.pot_bb, r.spr, r.equity[0], r.equity[1], r.realization[0], r.realization[1]] {
            put_f32(&mut out, x);
        }
    }
    out.extend_from_slice(&(sol.nodes.len() as u16).to_le_bytes());
    for node in &sol.nodes {
        put_f32(&mut out, node.reach);
    }
    for node in &sol.nodes {
        for f in &node.freq {
            out.extend(f.iter().map(|&x| (x.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    let rows: Vec<&Vec<f64>> = sol.nodes.iter().flat_map(|n| &n.ev).collect();
    let mut prev = [0i32; C];
    let mut evs: Vec<i16> = Vec::with_capacity(rows.len() * C);
    for row in rows {
        for (c, &ev) in row.iter().enumerate() {
            let q = (ev / EV_STEP_BB).round() as i32;
            if q.abs() > i16::MAX as i32 / 2 {
                return Err(format!("EV {ev} BB out of range"));
            }
            evs.push((q - prev[c]) as i16);
            prev[c] = q;
        }
    }
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
    fn steps(&mut self) -> Result<Vec<Step>, String> {
        let k = self.u8()? as usize;
        (0..k)
            .map(|_| {
                let seat = self.u8()? as usize;
                let kind = *KINDS.get(self.u8()? as usize).ok_or("unknown action kind")?;
                Ok(Step { seat, action: Action { kind, to_bb: self.f32()? } })
            })
            .collect()
    }
}

fn read_params(r: &mut Reader) -> Result<Params, String> {
    let n = r.u8()? as usize;
    let numbers = (0..Params::new(n, 1.0).numbers().len()).map(|_| r.f64()).collect::<Result<Vec<_>, _>>()?;
    let limps = r.u8()? != 0;
    let card_removal = r.u8()? != 0;
    let max_iterations = r.u32()?;
    let samples = r.u32()?;
    Params::from_numbers(n, &numbers, limps, card_removal, max_iterations, samples).ok_or_else(|| "bad parameters".into())
}

pub fn decode_entry(bytes: &[u8]) -> Result<Entry, String> {
    let mut r = Reader(bytes);
    let params = read_params(&mut r)?;
    let n = params.players;
    let shape = skeleton(&params).map_err(|e| e.to_string())?;
    let iterations = r.u32()?;
    let converged = r.u8()? != 0;
    let exploitability = r.f64()?;
    let ev = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
    let gain = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
    let ev_sum_error = r.f32()?;
    let realized = (0..r.u8()?)
        .map(|_| {
            let line = r.steps()?;
            let seats = vec![r.u8()? as usize, r.u8()? as usize];
            let x: Vec<f64> = (0..7).map(|_| r.f32()).collect::<Result<_, _>>()?;
            Ok(Realized {
                line,
                seats,
                reach: x[0],
                pot_bb: x[1],
                spr: x[2],
                equity: vec![x[3], x[4]],
                realization: vec![x[5], x[6]],
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let count = r.u16()? as usize;
    if count != shape.len() {
        return Err(format!("{count} nodes, the tree has {}", shape.len()));
    }
    let reach = (0..count).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
    let rows: usize = shape.iter().map(|s| s.2.len()).sum();
    let freq = r.take(rows * C)?;
    let lo = r.take(rows * C)?;
    let hi = r.take(rows * C)?;
    if !r.0.is_empty() {
        return Err("trailing bytes in entry".into());
    }
    let mut steps = [0i32; C];
    let mut row = 0;
    let mut nodes = Vec::with_capacity(count);
    for ((seat, line, actions), reach) in shape.into_iter().zip(reach) {
        let k = actions.len();
        let mut f = Vec::with_capacity(k);
        let mut e = Vec::with_capacity(k);
        for _ in 0..k {
            let range = row * C..(row + 1) * C;
            f.push(freq[range.clone()].iter().map(|&q| q as f64 / 255.0).collect());
            for (c, (&l, &h)) in lo[range.clone()].iter().zip(&hi[range]).enumerate() {
                steps[c] += i16::from_le_bytes([l, h]) as i32;
            }
            e.push(steps.iter().map(|&q| q as f64 * EV_STEP_BB).collect());
            row += 1;
        }
        nodes.push(Node { seat, line, actions, reach, freq: f, ev: e });
    }
    let solution = Solution {
        players: n,
        stack_bb: params.stack_bb,
        nodes,
        ev,
        gain,
        exploitability,
        iterations,
        converged,
        ev_sum_error,
        realized,
    };
    Ok(Entry { params, solution })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_round_trip_only_loses_quantisation() {
        let mut p = params(6, 15.0);
        p.multiway_samples = 64;
        p.max_iterations = 30;
        let sol = crate::preflop::solve(&p).unwrap();
        let bytes = encode_entry(&p, &sol).unwrap();
        let back = decode_entry(&bytes).unwrap();
        assert_eq!(back.params, p);
        let s = &back.solution;
        assert_eq!(s.nodes.len(), sol.nodes.len());
        for (x, y) in s.nodes.iter().zip(&sol.nodes) {
            assert_eq!((x.seat, &x.line, &x.actions), (y.seat, &y.line, &y.actions));
            for (fa, fb) in x.freq.iter().zip(&y.freq) {
                assert!(fa.iter().zip(fb).all(|(a, b)| (a - b).abs() <= 0.5 / 255.0 + 1e-9));
            }
            for (ea, eb) in x.ev.iter().zip(&y.ev) {
                assert!(ea.iter().zip(eb).all(|(a, b)| (a - b).abs() <= 0.5 * EV_STEP_BB + 1e-4));
            }
        }
        let file = write_file(6, &[bytes]);
        assert_eq!(read_file(&file, 6).unwrap().len(), 1);
        let mut broken = file.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(read_file(&broken, 6).is_err());
        assert!(read_file(&file, 9).is_err());
    }
}
