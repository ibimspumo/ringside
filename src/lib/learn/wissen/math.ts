/** Exact formulas behind the Wissen pages. Every number shown as a fact comes from here. */

/** Equity a call needs: call ÷ pot after the call. `pot` already contains the bet. */
export const neededEquity = (call: number, pot: number): number => call / (pot + call);

/** Facing a bet `bet` into `pot` (before the bet): B ÷ (P + 2B). */
export const potOdds = (pot: number, bet: number): number => bet / (pot + 2 * bet);

/** How often a pure bluff must work: B ÷ (P + B). */
export const alpha = (pot: number, bet: number): number => bet / (pot + bet);

/** Share of the range that must continue so pure bluffs break even: P ÷ (P + B). */
export const mdf = (pot: number, bet: number): number => pot / (pot + bet);

export function binom(n: number, k: number): number {
  if (k < 0 || k > n) return 0;
  let r = 1;
  for (let i = 1; i <= k; i++) r = (r * (n - k + i)) / i;
  return r;
}

/** Chance that at least one of `outs` arrives within `draws` cards from `unseen` cards. */
export const hitChance = (outs: number, draws: number, unseen: number): number =>
  1 - binom(unseen - outs, draws) / binom(unseen, draws);

/** Pocket pair flops a set or quads: 1 − C(48,3)/C(50,3). */
export const setOnFlop = (): number => 1 - binom(48, 3) / binom(50, 3);

/** Harrington's M: stack ÷ (small blind + big blind + all antes of one hand). */
export const mRatio = (stack: number, sb: number, bb: number, antesTotal: number): number => stack / (sb + bb + antesTotal);

/** Expected value of a list of outcomes. */
export const ev = (branches: { p: number; result: number }[]): number => branches.reduce((s, b) => s + b.p * b.result, 0);

/** Wilson 95 % interval for k hits in n tries. */
export function wilson(k: number, n: number, z = 1.96): [number, number] {
  const p = k / n;
  const d = 1 + (z * z) / n;
  const c = p + (z * z) / (2 * n);
  const m = z * Math.sqrt((p * (1 - p)) / n + (z * z) / (4 * n * n));
  return [(c - m) / d, (c + m) / d];
}

/**
 * Independent Chip Model (Malmuth-Harville): each player's expected share of the prize pool.
 * A player finishes first with probability stack ÷ total chips, recursively for every place.
 */
export function icm(stacks: number[], payouts: number[]): number[] {
  const n = stacks.length;
  const result = new Array(n).fill(0);
  const walk = (left: number[], place: number, prob: number) => {
    if (place >= payouts.length || left.length === 0) return;
    const total = left.reduce((s, i) => s + stacks[i], 0);
    for (const i of left) {
      if (stacks[i] <= 0) continue;
      const p = (prob * stacks[i]) / total;
      result[i] += p * payouts[place];
      walk(
        left.filter((j) => j !== i),
        place + 1,
        p,
      );
    }
  };
  walk(
    stacks.map((_, i) => i).filter((i) => stacks[i] > 0),
    0,
    1,
  );
  // Busted players still collect the lowest places that are left over.
  const busted = stacks.map((s, i) => (s <= 0 ? i : -1)).filter((i) => i >= 0);
  const alive = n - busted.length;
  busted.forEach((i, k) => (result[i] = payouts[alive + k] ?? 0));
  return result;
}

/* Formatting (German) ------------------------------------------------------------------------- */

const nf = (d: number) => new Intl.NumberFormat("de-DE", { minimumFractionDigits: d, maximumFractionDigits: d });

export const pct = (x: number, digits = 1): string => `${nf(digits).format(x * 100)} %`;
export const num = (x: number, digits = 1): string => nf(digits).format(x);
export const int = (x: number): string => nf(0).format(x);
