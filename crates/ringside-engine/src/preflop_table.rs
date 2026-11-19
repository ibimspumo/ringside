//! Precomputed solutions of the [`preflop`](crate::preflop) model, shipped with the
//! app: chip EV, equal stacks, each of the three ante structures of [`Ante`] on a fixed
//! stack grid ([`STACKS`]), in two families ([`Family`]):
//!
//! - **plain**: 2 to 6, 8 and 9 players ([`PLAYERS`]), limps from the small blind only,
//!   as the practice, the charts, the lessons and the spot library use them;
//! - **limps**: 2 to 6, 8 and 9 players ([`LIMP_PLAYERS`]) with limps from every seat
//!   ([`OPEN_LIMPS`]), for the hand analysis. Limping changes every range a little, so
//!   the plain family stays as it is for what is built on its exact ranges (the
//!   library's spots).
//!
//! A lookup only hits when the parameters equal the ones the table was solved with
//! ([`params`], [`limp_params`]). Frequencies are stored in 1/255 steps and action EVs
//! in [`EV_STEP_BB`] steps, so a hit differs from a fresh [`solve`](crate::preflop::solve)
//! by at most half a step.
//!
//! # File format
//! One file per family, ante structure and player count, regenerated with
//! `cargo run --release -p ringside-engine --bin gen-preflop-tables [-- --limps]`:
//!
//! ```text
//! header  magic "RSPR", format u32, ante u8, players u8, engine version (u8 length +
//!         UTF-8), entry count u32, body length u32, FNV-1a 64 of the body
//! body    deflate stream of the entries, each prefixed with its length (u32)
//! ```
//!
//! An entry holds the full solver parameters and the solution summary, then per node
//! its reach, the frequency of every action as bytes and the action EVs as i16 steps
//! (each minus the same class's previous stored EV), split into a low- and a high-byte
//! plane. Only classes in range at a node are stored: which ones follows from the
//! frequencies of the actor's earlier decisions ([`Solution::own_reach`]), and about
//! half of all cells are out of range. The tree itself is not stored either: it
//! follows from the parameters and is rebuilt on decoding. All numbers are
//! little-endian.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::idle::Idle;
use crate::preflop::{Action, Kind, Node, Params, Realized, Solution, Step, own_history, skeleton, taken};
pub use crate::pushfold_table::Ante;
use crate::range::NUM_CLASSES;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 4] = b"RSPR";
pub const FORMAT_VERSION: u32 = 3;
/// Resolution of stored action EVs.
pub const EV_STEP_BB: f64 = 0.01;
/// NashConv target of the tables, BB per hand summed over all players.
pub const TARGET_EXPLOITABILITY: f64 = 0.001;
pub const MAX_ITERATIONS: u32 = 4000;
/// Minimum limp share of the limp family, times [`limp_shape`](crate::preflop::limp_shape):
/// enough hands reach every spot behind a limp for its strategy to mean something.
pub const OPEN_LIMPS: f64 = 0.03;
pub const PLAYERS: [usize; 7] = [2, 3, 4, 5, 6, 8, 9];
pub const LIMP_PLAYERS: [usize; 7] = [2, 3, 4, 5, 6, 8, 9];
/// Stack depths in BB.
pub const STACKS: [f64; 8] = [15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 75.0, 100.0];
const C: usize = NUM_CLASSES;
const STALE: &str = "run `cargo run --release -p ringside-engine --bin gen-preflop-tables`";

/// A set of tables, see the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Plain,
    Limps,
}

impl Family {
    pub const ALL: [Family; 2] = [Family::Plain, Family::Limps];

    pub fn players(self) -> &'static [usize] {
        match self {
            Family::Plain => &PLAYERS,
            Family::Limps => &LIMP_PLAYERS,
        }
    }

    /// Prefix of the family's file names after "preflop_".
    pub fn key(self) -> &'static str {
        match self {
            Family::Plain => "",
            Family::Limps => "limps_",
        }
    }

    fn of(p: &Params) -> Family {
        if p.open_limps > 0.0 { Family::Limps } else { Family::Plain }
    }

    /// The parameters an entry of this family was solved with.
    pub fn params(self, ante: Ante, players: usize, stack_bb: f64) -> Params {
        match self {
            Family::Plain => params(ante, players, stack_bb),
            Family::Limps => limp_params(ante, players, stack_bb),
        }
    }
}

/// A heads-up flop of a table named without its ranges: the table and the non-fold
/// actions on the way in model seats. The ranges follow from them, so Ringside Cloud
/// shares this instead of ranges, and the verifier rebuilds them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelLine {
    pub family: Family,
    pub ante: Ante,
    pub players: usize,
    pub stack_bb: f64,
    pub line: Vec<Step>,
}

impl ModelLine {
    /// The table's parameters.
    pub fn params(&self) -> Params {
        self.family.params(self.ante, self.players, self.stack_bb)
    }
}

/// The parameters a plain table entry was solved with.
pub fn params(ante: Ante, players: usize, stack_bb: f64) -> Params {
    let mut p = Params::new(players, stack_bb);
    (p.ante_bb, p.bb_ante_bb) = ante.amounts();
    p.target_exploitability = TARGET_EXPLOITABILITY;
    p.max_iterations = MAX_ITERATIONS;
    p
}

/// The parameters a table entry with limps from every seat was solved with.
pub fn limp_params(ante: Ante, players: usize, stack_bb: f64) -> Params {
    let mut p = params(ante, players, stack_bb);
    p.open_limps = OPEN_LIMPS;
    p
}

fn ante_of(p: &Params) -> Option<Ante> {
    Ante::ALL.into_iter().find(|a| a.amounts() == (p.ante_bb, p.bb_ante_bb))
}

/// The precomputed solution for exactly these parameters.
pub fn lookup(p: &Params) -> Option<Solution> {
    let ante = ante_of(p)?;
    let family = Family::of(p);
    let table = table(family, ante, p.players)?;
    let bytes = table.index.get(&p.stack_bb.to_bits())?;
    let entry = decode_entry(bytes).unwrap_or_else(|e| panic!("preflop table {}{} {}: {e}; {STALE}", family.key(), ante.key(), p.players));
    (entry.params == *p).then_some(entry.solution)
}

/// The plain table solution at `players` × `stack_bb` with `ante`.
pub fn chart(ante: Ante, players: usize, stack_bb: f64) -> Option<Solution> {
    lookup(&params(ante, players, stack_bb))
}

// Embedded tables.

struct Table {
    index: HashMap<u64, Vec<u8>>,
}

/// By ante code, then player count as in [`PLAYERS`].
static PLAIN_BYTES: [[&[u8]; 7]; 3] = [
    [
        include_bytes!("../data/preflop_none_2max.bin"),
        include_bytes!("../data/preflop_none_3max.bin"),
        include_bytes!("../data/preflop_none_4max.bin"),
        include_bytes!("../data/preflop_none_5max.bin"),
        include_bytes!("../data/preflop_none_6max.bin"),
        include_bytes!("../data/preflop_none_8max.bin"),
        include_bytes!("../data/preflop_none_9max.bin"),
    ],
    [
        include_bytes!("../data/preflop_classic_2max.bin"),
        include_bytes!("../data/preflop_classic_3max.bin"),
        include_bytes!("../data/preflop_classic_4max.bin"),
        include_bytes!("../data/preflop_classic_5max.bin"),
        include_bytes!("../data/preflop_classic_6max.bin"),
        include_bytes!("../data/preflop_classic_8max.bin"),
        include_bytes!("../data/preflop_classic_9max.bin"),
    ],
    [
        include_bytes!("../data/preflop_big_blind_2max.bin"),
        include_bytes!("../data/preflop_big_blind_3max.bin"),
        include_bytes!("../data/preflop_big_blind_4max.bin"),
        include_bytes!("../data/preflop_big_blind_5max.bin"),
        include_bytes!("../data/preflop_big_blind_6max.bin"),
        include_bytes!("../data/preflop_big_blind_8max.bin"),
        include_bytes!("../data/preflop_big_blind_9max.bin"),
    ],
];

/// By ante code, then player count as in [`LIMP_PLAYERS`].
static LIMP_BYTES: [[&[u8]; 7]; 3] = [
    [
        include_bytes!("../data/preflop_limps_none_2max.bin"),
        include_bytes!("../data/preflop_limps_none_3max.bin"),
        include_bytes!("../data/preflop_limps_none_4max.bin"),
        include_bytes!("../data/preflop_limps_none_5max.bin"),
        include_bytes!("../data/preflop_limps_none_6max.bin"),
        include_bytes!("../data/preflop_limps_none_8max.bin"),
        include_bytes!("../data/preflop_limps_none_9max.bin"),
    ],
    [
        include_bytes!("../data/preflop_limps_classic_2max.bin"),
        include_bytes!("../data/preflop_limps_classic_3max.bin"),
        include_bytes!("../data/preflop_limps_classic_4max.bin"),
        include_bytes!("../data/preflop_limps_classic_5max.bin"),
        include_bytes!("../data/preflop_limps_classic_6max.bin"),
        include_bytes!("../data/preflop_limps_classic_8max.bin"),
        include_bytes!("../data/preflop_limps_classic_9max.bin"),
    ],
    [
        include_bytes!("../data/preflop_limps_big_blind_2max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_3max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_4max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_5max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_6max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_8max.bin"),
        include_bytes!("../data/preflop_limps_big_blind_9max.bin"),
    ],
];

/// Inflated on first use, one file at a time.
static PLAIN_TABLES: [[Idle<Table>; 7]; 3] = {
    fn at<const A: usize, const N: usize>() -> Table {
        load(Ante::ALL[A], PLAYERS[N], PLAIN_BYTES[A][N])
    }
    [
        [Idle::new(at::<0, 0>), Idle::new(at::<0, 1>), Idle::new(at::<0, 2>), Idle::new(at::<0, 3>), Idle::new(at::<0, 4>), Idle::new(at::<0, 5>), Idle::new(at::<0, 6>)],
        [Idle::new(at::<1, 0>), Idle::new(at::<1, 1>), Idle::new(at::<1, 2>), Idle::new(at::<1, 3>), Idle::new(at::<1, 4>), Idle::new(at::<1, 5>), Idle::new(at::<1, 6>)],
        [Idle::new(at::<2, 0>), Idle::new(at::<2, 1>), Idle::new(at::<2, 2>), Idle::new(at::<2, 3>), Idle::new(at::<2, 4>), Idle::new(at::<2, 5>), Idle::new(at::<2, 6>)],
    ]
};

static LIMP_TABLES: [[Idle<Table>; 7]; 3] = {
    fn at<const A: usize, const N: usize>() -> Table {
        load(Ante::ALL[A], LIMP_PLAYERS[N], LIMP_BYTES[A][N])
    }
    [
        [Idle::new(at::<0, 0>), Idle::new(at::<0, 1>), Idle::new(at::<0, 2>), Idle::new(at::<0, 3>), Idle::new(at::<0, 4>), Idle::new(at::<0, 5>), Idle::new(at::<0, 6>)],
        [Idle::new(at::<1, 0>), Idle::new(at::<1, 1>), Idle::new(at::<1, 2>), Idle::new(at::<1, 3>), Idle::new(at::<1, 4>), Idle::new(at::<1, 5>), Idle::new(at::<1, 6>)],
        [Idle::new(at::<2, 0>), Idle::new(at::<2, 1>), Idle::new(at::<2, 2>), Idle::new(at::<2, 3>), Idle::new(at::<2, 4>), Idle::new(at::<2, 5>), Idle::new(at::<2, 6>)],
    ]
};

fn load(ante: Ante, players: usize, bytes: &[u8]) -> Table {
    let entries = read_file(bytes, ante, players).unwrap_or_else(|e| panic!("preflop table {} {players} is invalid ({e}); {STALE}", ante.key()));
    let index = entries
        .into_iter()
        .map(|e| {
            let head = read_params(&mut Reader(&e)).expect("entry checked while reading the file");
            (head.stack_bb.to_bits(), e)
        })
        .collect();
    Table { index }
}

fn slot(family: Family, players: usize) -> Option<usize> {
    family.players().iter().position(|&n| n == players)
}

fn table(family: Family, ante: Ante, players: usize) -> Option<Arc<Table>> {
    let a = ante.code() as usize;
    slot(family, players).map(|i| match family {
        Family::Plain => PLAIN_TABLES[a][i].get(),
        Family::Limps => LIMP_TABLES[a][i].get(),
    })
}

/// Drops the inflated tables unused for `idle`.
pub(crate) fn release(idle: Duration) -> usize {
    PLAIN_TABLES.iter().flatten().chain(LIMP_TABLES.iter().flatten()).filter(|t| t.release(idle)).count()
}

/// The embedded file of a family, ante structure and player count (for the generator and tests).
pub fn embedded_file(family: Family, ante: Ante, players: usize) -> Option<&'static [u8]> {
    let a = ante.code() as usize;
    slot(family, players).map(|i| match family {
        Family::Plain => PLAIN_BYTES[a][i],
        Family::Limps => LIMP_BYTES[a][i],
    })
}

/// Every entry of a family, ante structure and player count, by stack.
pub fn embedded_entries(family: Family, ante: Ante, players: usize) -> Vec<(Params, Solution)> {
    let Some(table) = table(family, ante, players) else { return Vec::new() };
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

// Encoding.

pub struct Entry {
    pub params: Params,
    pub solution: Solution,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// A whole table file from encoded entries (see [`encode_entry`]).
pub fn write_file(ante: Ante, players: usize, entries: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for e in entries {
        body.extend_from_slice(&(e.len() as u32).to_le_bytes());
        body.extend_from_slice(e);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.push(ante.code());
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
    pub ante: Ante,
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
    let ante = *Ante::ALL.get(r.u8()? as usize).ok_or("unknown ante")?;
    let players = r.u8()? as usize;
    let len = r.u8()? as usize;
    let engine_version = String::from_utf8(r.take(len)?.to_vec()).map_err(|e| e.to_string())?;
    let entries = r.u32()? as usize;
    let body_len = r.u32()? as usize;
    let checksum = r.u64()?;
    Ok(FileInfo { ante, players, engine_version, entries, body_len, checksum })
}

fn read_file(bytes: &[u8], ante: Ante, players: usize) -> Result<Vec<Vec<u8>>, String> {
    let info = file_info(bytes)?;
    if (info.ante, info.players) != (ante, players) {
        return Err(format!("holds {} {} players, expected {} {players}", info.ante.key(), info.players, ante.key()));
    }
    let header = 4 + 4 + 1 + 1 + 1 + info.engine_version.len() + 4 + 4 + 8;
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
    let in_range: Vec<Vec<bool>> = sol.own_reach().iter().map(|r| r.iter().map(|&x| x > 0.0).collect()).collect();
    let mut freq = Vec::new();
    let mut evs: Vec<i16> = Vec::new();
    let mut prev = [0i32; C];
    for (node, live) in sol.nodes.iter().zip(&in_range) {
        for (f, e) in node.freq.iter().zip(&node.ev) {
            for c in (0..C).filter(|&c| live[c]) {
                freq.push(quantise(f[c]));
                let q = (e[c] / EV_STEP_BB).round() as i32;
                if q.abs() > i16::MAX as i32 / 2 {
                    return Err(format!("EV {} BB out of range", e[c]));
                }
                evs.push((q - prev[c]) as i16);
                prev[c] = q;
            }
        }
    }
    out.extend_from_slice(&(freq.len() as u32).to_le_bytes());
    out.extend(freq);
    out.extend(evs.iter().map(|v| v.to_le_bytes()[0]));
    out.extend(evs.iter().map(|v| v.to_le_bytes()[1]));
    Ok(out)
}

fn quantise(freq: f64) -> u8 {
    (freq.clamp(0.0, 1.0) * 255.0).round() as u8
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
    let cells = r.u32()? as usize;
    let freq = r.take(cells)?;
    let lo = r.take(cells)?;
    let hi = r.take(cells)?;
    if !r.0.is_empty() {
        return Err("trailing bytes in entry".into());
    }
    let shapes: Vec<_> = shape.iter().map(|(seat, line, actions)| (*seat, line.as_slice(), actions.as_slice())).collect();
    let history = own_history(&shapes);
    let mut prev = [0i32; C];
    let mut cell = 0;
    let mut nodes: Vec<Node> = Vec::with_capacity(count);
    for (((seat, line, actions), reach), history) in shape.into_iter().zip(reach).zip(history) {
        let live: Vec<bool> = (0..C).map(|c| history.iter().all(|&(u, a)| taken(nodes[u].freq[a][c]))).collect();
        let k = actions.len();
        let mut f = vec![vec![0.0; C]; k];
        let mut e = vec![vec![0.0; C]; k];
        for a in 0..k {
            for c in (0..C).filter(|&c| live[c]) {
                let (&q, &l, &h) = (freq.get(cell).ok_or("too few cells")?, &lo[cell], &hi[cell]);
                f[a][c] = q as f64 / 255.0;
                prev[c] += i16::from_le_bytes([l, h]) as i32;
                e[a][c] = prev[c] as f64 * EV_STEP_BB;
                cell += 1;
            }
        }
        nodes.push(Node { seat, line, actions, reach, freq: f, ev: e });
    }
    if cell != cells {
        return Err(format!("{cells} cells stored, the ranges hold {cell}"));
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
        let mut p = params(Ante::Classic, 6, 15.0);
        p.multiway_samples = 64;
        p.max_iterations = 30;
        let sol = crate::preflop::solve(&p).unwrap();
        let bytes = encode_entry(&p, &sol).unwrap();
        let back = decode_entry(&bytes).unwrap();
        assert_eq!(back.params, p);
        let s = &back.solution;
        assert_eq!(s.nodes.len(), sol.nodes.len());
        let mut out_of_range = 0;
        for ((x, y), reach) in s.nodes.iter().zip(&sol.nodes).zip(sol.own_reach()) {
            assert_eq!((x.seat, &x.line, &x.actions), (y.seat, &y.line, &y.actions));
            out_of_range += reach.iter().filter(|&&r| r == 0.0).count();
            for (fa, fb) in x.freq.iter().zip(&y.freq) {
                assert!(fa.iter().zip(fb).all(|(a, b)| (a - b).abs() <= 0.5 / 255.0 + 1e-9));
            }
            for (ea, eb) in x.ev.iter().zip(&y.ev) {
                assert!(ea.iter().zip(eb).all(|(a, b)| (a - b).abs() <= 0.5 * EV_STEP_BB + 1e-4));
            }
        }
        // Out-of-range classes are cleared by the solver and skipped by the format.
        assert!(out_of_range > 0);
        let file = write_file(Ante::Classic, 6, &[bytes]);
        assert_eq!(read_file(&file, Ante::Classic, 6).unwrap().len(), 1);
        let mut broken = file.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(read_file(&broken, Ante::Classic, 6).is_err());
        assert!(read_file(&file, Ante::Classic, 9).is_err());
        assert!(read_file(&file, Ante::None, 6).is_err());
    }
}
