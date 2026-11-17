//! Heads-up push/fold Nash equilibrium in chip EV: the small blind shoves or folds,
//! the big blind calls or folds.
//!
//! Dead money (antes of folded players, a big-blind ante) goes to whoever wins the pot,
//! so the two results always add up to it: the game is constant-sum, which has the
//! same equilibria as the zero-sum game the solver works with.
//!
//! Card removal: strategies are per class, and the joint deal probability of SB class
//! A and BB class B is `class_pair_count(A, B) / (1326 · 1225)`, using the matrix's
//! equity averaged over exactly those disjoint combo pairs. By suit symmetry every
//! combo of a class has the same number of compatible opponent combos per class and
//! the same average equity against them, so this is not an approximation: class-level
//! play loses nothing against combo-level play.
//!
//! Solving runs in two steps. CFR+ (regret matching+ with alternating updates and
//! linear averaging, a fast-converging relative of fictitious play) gets close to the
//! equilibrium. Its EV gaps then tell which classes are clearly pure; the few classes
//! closest to indifference are treated as mixed, and their exact frequencies follow
//! from the indifference equations of the opponent's mixed classes (a k×k linear
//! system per player). If that exact profile checks out it is returned, otherwise the
//! CFR+ average.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::preflop_matrix::PreflopMatrix;
use crate::range::{HandClass, NUM_CLASSES, Range, class_pair_count};

const N: usize = NUM_CLASSES;
const CHECK_EVERY: u32 = 64;
/// Most mixed classes per player tried when solving for the exact equilibrium.
const MAX_MIXED: usize = 4;
/// NashConv below which the refined profile counts as exact (float noise level).
const EXACT: f64 = 1e-10;
/// Frequencies this close to 0 or 1 in the CFR+ fallback are reported as pure.
const SNAP: f64 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PushFoldParams {
    /// Effective stack in big blinds, including blinds and ante the player posts.
    pub stack_bb: f64,
    pub small_blind_bb: f64,
    /// Ante paid by each of the two players.
    pub ante_bb: f64,
    /// Money already in the pot that neither player can lose any more: antes of the
    /// folded players, a big-blind ante. Whoever wins the pot takes it.
    #[serde(default)]
    pub dead_bb: f64,
    /// CFR+ runs until the summed gain of both best responses (NashConv) is below this.
    pub target_exploitability_bb: f64,
    pub max_iterations: u32,
}

impl PushFoldParams {
    pub fn new(stack_bb: f64) -> PushFoldParams {
        PushFoldParams {
            stack_bb,
            small_blind_bb: 0.5,
            ante_bb: 0.0,
            dead_bb: 0.0,
            target_exploitability_bb: 1e-6,
            max_iterations: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PushFoldResult {
    /// Shove frequency per class, indexed by `HandClass::index`.
    pub shove: Vec<f64>,
    /// Call frequency per class.
    pub call: Vec<f64>,
    /// Share of all 1326 combos the SB shoves, in percent.
    pub shove_pct: f64,
    /// Share of all 1326 combos the BB calls a shove with, in percent.
    pub call_pct: f64,
    /// SB expected result per hand in BB (the BB's is the negative).
    pub sb_ev_bb: f64,
    /// What the SB could gain per hand by switching to a best response.
    pub sb_gain_bb: f64,
    /// What the BB could gain per hand by switching to a best response.
    pub bb_gain_bb: f64,
    /// Whether the mixed frequencies were solved exactly (else CFR+ averages).
    pub exact: bool,
    pub iterations: u32,
    /// SB result of shoving (`act`) and folding per class against the BB's calls, in BB
    /// per hand of that class, counted from before the blinds and antes were posted.
    pub sb_class_ev: Vec<ClassEv>,
    /// BB result of calling (`act`) and folding per class once the SB has shoved, in BB,
    /// counted like `sb_class_ev`. Zero for classes the SB's shoves never meet.
    pub bb_class_ev: Vec<ClassEv>,
}

/// Expected results of a class's two options, in BB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassEv {
    pub act: f64,
    pub fold: f64,
}

impl ClassEv {
    /// What choosing `act` (shove/call) or not costs against the better option, ≥ 0.
    pub fn loss(&self, act: bool) -> f64 {
        let chosen = if act { self.act } else { self.fold };
        (self.act.max(self.fold) - chosen).max(0.0)
    }
}

impl PushFoldResult {
    pub fn exploitability_bb(&self) -> f64 {
        self.sb_gain_bb + self.bb_gain_bb
    }

    pub fn shove_range(&self) -> Range {
        to_range(&self.shove)
    }

    pub fn call_range(&self) -> Range {
        to_range(&self.call)
    }

    /// Classes played at least half of the time.
    pub fn shove_classes(&self) -> Vec<HandClass> {
        HandClass::all()
            .filter(|c| self.shove[c.index()] >= 0.5)
            .collect()
    }

    pub fn call_classes(&self) -> Vec<HandClass> {
        HandClass::all()
            .filter(|c| self.call[c.index()] >= 0.5)
            .collect()
    }
}

fn to_range(freq: &[f64]) -> Range {
    let mut r = Range::empty();
    for c in HandClass::all() {
        r.set_class(c, freq[c.index()] as f32);
    }
    r
}

/// Payoffs from the SB's point of view; row index = SB class, column = BB class.
struct Game {
    /// Joint probability of the deal (SB class a, BB class b). Symmetric.
    joint: Vec<f64>,
    /// `joint · (showdown − steal)`: what a BB call changes for the SB versus a fold.
    call_delta: Vec<f64>,
    class_prob: Vec<f64>,
    fold_loss: f64,
    steal: f64,
    dead: f64,
}

/// (act, pass) values per class for one player: shove/fold or call/fold.
type Values = Vec<(f64, f64)>;

impl Game {
    fn new(p: &PushFoldParams, matrix: &PreflopMatrix) -> Game {
        let steal = 1.0 + p.ante_bb + p.dead_bb;
        let mut joint = vec![0.0; N * N];
        let mut call_delta = vec![0.0; N * N];
        for a in HandClass::all() {
            for b in HandClass::all() {
                let i = a.index() * N + b.index();
                joint[i] = class_pair_count(a, b) as f64 / (1326.0 * 1225.0);
                let eq = matrix.equity(a, b);
                let showdown = (2.0 * eq - 1.0) * p.stack_bb + eq * p.dead_bb;
                call_delta[i] = joint[i] * (showdown - steal);
            }
        }
        Game {
            joint,
            call_delta,
            class_prob: HandClass::all()
                .map(|c| c.combo_count() as f64 / 1326.0)
                .collect(),
            fold_loss: p.small_blind_bb + p.ante_bb,
            steal,
            dead: p.dead_bb,
        }
    }

    /// SB (shove, fold) values against call frequencies.
    fn sb_values(&self, call: &[f64], out: &mut Values) {
        for (a, o) in out.iter_mut().enumerate() {
            let row = &self.call_delta[a * N..][..N];
            let mut delta = 0.0;
            for b in 0..N {
                delta += row[b] * call[b];
            }
            let p = self.class_prob[a];
            *o = (self.steal * p + delta, -self.fold_loss * p);
        }
    }

    /// BB (call, fold) values against shove frequencies, from the BB's point of view,
    /// leaving out the SB-folds part and the dead money the BB can't influence.
    fn bb_values(&self, shove: &[f64], out: &mut Values) {
        out.fill((0.0, 0.0));
        for (a, &s) in shove.iter().enumerate() {
            if s == 0.0 {
                continue;
            }
            let (j, d) = (&self.joint[a * N..][..N], &self.call_delta[a * N..][..N]);
            for b in 0..N {
                let fold = -s * self.steal * j[b];
                out[b].0 += fold - s * d[b];
                out[b].1 += fold;
            }
        }
    }

    /// (SB EV, SB gain, BB gain) of a strategy profile.
    fn evaluate(&self, shove: &[f64], call: &[f64]) -> (f64, f64, f64) {
        let mut sb = vec![(0.0, 0.0); N];
        let mut bb = vec![(0.0, 0.0); N];
        self.sb_values(call, &mut sb);
        self.bb_values(shove, &mut bb);
        let mix = |v: &Values, f: &[f64]| -> f64 {
            v.iter()
                .zip(f)
                .map(|(v, f)| f * v.0 + (1.0 - f) * v.1)
                .sum()
        };
        let best = |v: &Values| -> f64 { v.iter().map(|v| v.0.max(v.1)).sum() };
        let ev = mix(&sb, shove);
        (ev, best(&sb) - ev, best(&bb) - mix(&bb, call))
    }

    /// Per-class EVs of both players, see [`PushFoldResult::sb_class_ev`].
    fn class_evs(&self, shove: &[f64], call: &[f64]) -> (Vec<ClassEv>, Vec<ClassEv>) {
        let (mut sb, mut bb) = (vec![(0.0, 0.0); N], vec![(0.0, 0.0); N]);
        self.sb_values(call, &mut sb);
        self.bb_values(shove, &mut bb);
        let sb = sb
            .iter()
            .zip(&self.class_prob)
            .map(|(v, p)| ClassEv { act: v.0 / p, fold: v.1 / p })
            .collect();
        let bb = bb
            .iter()
            .enumerate()
            .map(|(b, v)| {
                let reach: f64 = (0..N).map(|a| shove[a] * self.joint[a * N + b]).sum();
                if reach > 0.0 {
                    ClassEv { act: v.0 / reach + self.dead, fold: v.1 / reach + self.dead }
                } else {
                    ClassEv { act: 0.0, fold: 0.0 }
                }
            })
            .collect();
        (sb, bb)
    }

    /// Exact equilibrium with the `k` classes closest to indifference mixed, if that
    /// support works out.
    fn refine(&self, shove: &[f64], call: &[f64]) -> Option<(Vec<f64>, Vec<f64>)> {
        let (mut sb, mut bb) = (vec![(0.0, 0.0); N], vec![(0.0, 0.0); N]);
        self.sb_values(call, &mut sb);
        self.bb_values(shove, &mut bb);
        let gaps = |v: &Values| -> Vec<f64> {
            v.iter()
                .zip(&self.class_prob)
                .map(|(v, p)| (v.0 - v.1) / p)
                .collect()
        };
        let (sb_gap, bb_gap) = (gaps(&sb), gaps(&bb));
        let by_closeness = |g: &[f64]| {
            let mut idx: Vec<usize> = (0..N).collect();
            idx.sort_by(|&x, &y| g[x].abs().total_cmp(&g[y].abs()));
            idx
        };
        let (sb_order, bb_order) = (by_closeness(&sb_gap), by_closeness(&bb_gap));
        let pure = |g: &[f64]| -> Vec<f64> {
            g.iter().map(|&x| if x > 0.0 { 1.0 } else { 0.0 }).collect()
        };

        for k in 0..=MAX_MIXED {
            let (m_sb, m_bb) = (&sb_order[..k], &bb_order[..k]);
            let (mut s, mut c) = (pure(&sb_gap), pure(&bb_gap));
            if k > 0 {
                // SB indifference on its mixed classes fixes the BB's mixed calls.
                for &b in m_bb {
                    c[b] = 0.0;
                }
                let mut sys = vec![0.0; k * k];
                let mut rhs = vec![0.0; k];
                for (i, &a) in m_sb.iter().enumerate() {
                    let row = &self.call_delta[a * N..][..N];
                    for (j, &b) in m_bb.iter().enumerate() {
                        sys[i * k + j] = row[b];
                    }
                    let fixed: f64 = (0..N).map(|b| row[b] * c[b]).sum();
                    rhs[i] = -((self.steal + self.fold_loss) * self.class_prob[a] + fixed);
                }
                let xs = solve_linear(sys, rhs, k)?;
                // BB indifference on its mixed classes fixes the SB's mixed shoves.
                for &a in m_sb {
                    s[a] = 0.0;
                }
                let mut sys = vec![0.0; k * k];
                let mut rhs = vec![0.0; k];
                for (i, &b) in m_bb.iter().enumerate() {
                    for (j, &a) in m_sb.iter().enumerate() {
                        sys[i * k + j] = self.call_delta[a * N + b];
                    }
                    rhs[i] = -(0..N)
                        .map(|a| self.call_delta[a * N + b] * s[a])
                        .sum::<f64>();
                }
                let ys = solve_linear(sys, rhs, k)?;
                let valid = |v: &[f64]| v.iter().all(|x| (-1e-12..=1.0 + 1e-12).contains(x));
                if !valid(&xs) || !valid(&ys) {
                    continue;
                }
                for (&b, x) in m_bb.iter().zip(xs) {
                    c[b] = x.clamp(0.0, 1.0);
                }
                for (&a, y) in m_sb.iter().zip(ys) {
                    s[a] = y.clamp(0.0, 1.0);
                }
            }
            let (_, g1, g2) = self.evaluate(&s, &c);
            if g1 + g2 < EXACT {
                return Some((s, c));
            }
        }
        None
    }
}

/// Gaussian elimination with partial pivoting; `None` if (nearly) singular.
fn solve_linear(mut a: Vec<f64>, mut b: Vec<f64>, k: usize) -> Option<Vec<f64>> {
    for col in 0..k {
        let pivot =
            (col..k).max_by(|&x, &y| a[x * k + col].abs().total_cmp(&a[y * k + col].abs()))?;
        if a[pivot * k + col].abs() < 1e-18 {
            return None;
        }
        for j in 0..k {
            a.swap(col * k + j, pivot * k + j);
        }
        b.swap(col, pivot);
        for row in col + 1..k {
            let f = a[row * k + col] / a[col * k + col];
            for j in col..k {
                a[row * k + j] -= f * a[col * k + j];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = vec![0.0; k];
    for row in (0..k).rev() {
        let tail: f64 = (row + 1..k).map(|j| a[row * k + j] * x[j]).sum();
        x[row] = (b[row] - tail) / a[row * k + row];
    }
    Some(x)
}

/// Regret-matching+ state for one player's 169 binary decisions.
struct Learner {
    regret: Vec<(f64, f64)>,
    current: Vec<f64>,
    average: Vec<f64>,
    weight: f64,
}

impl Learner {
    fn new() -> Learner {
        Learner {
            regret: vec![(0.0, 0.0); N],
            current: vec![0.5; N],
            average: vec![0.0; N],
            weight: 0.0,
        }
    }

    fn update(&mut self, values: &Values, t: f64) {
        for ((r, cur), &(act, pass)) in self.regret.iter_mut().zip(&mut self.current).zip(values) {
            let v = *cur * act + (1.0 - *cur) * pass;
            r.0 = (r.0 + act - v).max(0.0);
            r.1 = (r.1 + pass - v).max(0.0);
            let sum = r.0 + r.1;
            *cur = if sum > 0.0 { r.0 / sum } else { 0.5 };
        }
        for (avg, &cur) in self.average.iter_mut().zip(&self.current) {
            *avg += t * cur;
        }
        self.weight += t;
    }

    fn average(&self) -> Vec<f64> {
        self.average.iter().map(|a| a / self.weight).collect()
    }
}

/// Linear averaging keeps a vanishing trace of the uniform early iterations in every
/// class; this snaps it away.
fn snap(freqs: &[f64]) -> Vec<f64> {
    freqs
        .iter()
        .map(|&f| {
            if f < SNAP {
                0.0
            } else if f > 1.0 - SNAP {
                1.0
            } else {
                f
            }
        })
        .collect()
}

pub fn solve_push_fold(params: &PushFoldParams) -> Result<PushFoldResult, Error> {
    solve_with_matrix(params, PreflopMatrix::embedded())
}

pub fn solve_with_matrix(
    p: &PushFoldParams,
    matrix: &PreflopMatrix,
) -> Result<PushFoldResult, Error> {
    let numbers = [
        p.stack_bb,
        p.small_blind_bb,
        p.ante_bb,
        p.dead_bb,
        p.target_exploitability_bb,
    ];
    if numbers.iter().any(|x| !x.is_finite()) || p.small_blind_bb <= 0.0 || p.small_blind_bb > 1.0 {
        return Err(Error::InvalidInput(
            "small blind must be in (0, 1] BB".into(),
        ));
    }
    if p.dead_bb < 0.0 {
        return Err(Error::InvalidInput("dead money must not be negative".into()));
    }
    if p.ante_bb < 0.0 || p.stack_bb < 1.0 + p.ante_bb {
        return Err(Error::InvalidInput(
            "stack must cover the big blind plus ante".into(),
        ));
    }
    let game = Game::new(p, matrix);
    let (mut sb, mut bb) = (Learner::new(), Learner::new());
    let (mut sb_vals, mut bb_vals) = (vec![(0.0, 0.0); N], vec![(0.0, 0.0); N]);
    let mut iterations = 0;
    while iterations < p.max_iterations {
        iterations += 1;
        let t = iterations as f64;
        game.sb_values(&bb.current, &mut sb_vals);
        sb.update(&sb_vals, t);
        game.bb_values(&sb.current, &mut bb_vals);
        bb.update(&bb_vals, t);
        if iterations % CHECK_EVERY == 0 {
            let (_, g1, g2) = game.evaluate(&sb.average(), &bb.average());
            if g1 + g2 < p.target_exploitability_bb {
                break;
            }
        }
    }

    let (avg_shove, avg_call) = (sb.average(), bb.average());
    let (shove, call, exact) = match game.refine(&avg_shove, &avg_call) {
        Some((s, c)) => (s, c, true),
        None => (snap(&avg_shove), snap(&avg_call), false),
    };
    let (ev, sb_gain, bb_gain) = game.evaluate(&shove, &call);
    let (sb_class_ev, bb_class_ev) = game.class_evs(&shove, &call);
    let pct = |f: &[f64]| {
        100.0
            * f.iter()
                .zip(&game.class_prob)
                .map(|(f, p)| f * p)
                .sum::<f64>()
    };
    Ok(PushFoldResult {
        shove_pct: pct(&shove),
        call_pct: pct(&call),
        shove,
        call,
        sb_ev_bb: ev,
        sb_gain_bb: sb_gain.max(0.0),
        bb_gain_bb: bb_gain.max(0.0),
        exact,
        iterations,
        sb_class_ev,
        bb_class_ev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_solver() {
        let x = solve_linear(vec![0.0, 2.0, 3.0, 1.0], vec![4.0, 5.0], 2).unwrap();
        assert!(
            (x[0] - 1.0).abs() < 1e-12 && (x[1] - 2.0).abs() < 1e-12,
            "{x:?}"
        );
        assert!(solve_linear(vec![1.0, 2.0, 2.0, 4.0], vec![1.0, 2.0], 2).is_none());
    }
}
