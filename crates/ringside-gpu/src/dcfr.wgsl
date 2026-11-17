// Discounted CFR, one tree level per dispatch. The same update as postflop-solver's
// `solve_recursive`, but breadth first: a forward pass spreads the opponent's reach down the tree,
// the terminals turn it into values, a backward pass folds values up and updates regrets.

struct Params {
    traverser: u32,
    list_offset: u32,
    list_count: u32,
    n_trav: u32,
    n_opp: u32,
    stride: u32,
    num_nodes: u32,
    alpha: f32,
    beta: f32,
    gamma: f32,
    _pad: u32,
}

// Node record, see tree.rs.
const NODE_WORDS: u32 = 10u;
const KIND: u32 = 0u;
const PLAYER: u32 = 1u;
const PARENT: u32 = 2u;
const ACTION: u32 = 3u;
const CHILDREN: u32 = 4u;
const NUM_CHILDREN: u32 = 5u;
const STORE: u32 = 6u;
const VALUE: u32 = 7u;
const BLOCK: u32 = 8u;

const DECISION: u32 = 0u;
const FOLD: u32 = 1u;
const CHANCE: u32 = 3u;
const NO_PARENT: u32 = 0xffffffffu;
const MAX_ACTIONS: u32 = 8u;

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> topo: array<u32>;
// Opponent reach per node and hand; rows `num_nodes` and `num_nodes + 1` hold the initial weights.
@group(0) @binding(2) var<storage, read_write> reach: array<f32>;
@group(0) @binding(3) var<storage, read_write> cfv: array<f32>;
@group(0) @binding(4) var<storage, read_write> regret: array<f32>;
@group(0) @binding(5) var<storage, read_write> strat_sum: array<f32>;
// Sorted hand lists and per-hand records for each board, see `showdown_block` in tree.rs.
@group(0) @binding(6) var<storage, read> sd: array<u32>;

fn field(n: u32, f: u32) -> u32 {
    return topo[n * NODE_WORDS + f];
}

// Regret matching for one hand; uniform when no action has positive regret.
fn probability(store: u32, num_actions: u32, num_hands: u32, action: u32, h: u32) -> f32 {
    var sum = 0.0;
    for (var a = 0u; a < num_actions; a++) {
        sum += max(regret[store + a * num_hands + h], 0.0);
    }
    if (sum == 0.0) {
        return 1.0 / f32(num_actions);
    }
    return max(regret[store + action * num_hands + h], 0.0) / sum;
}

@compute @workgroup_size(64)
fn forward(@builtin(global_invocation_id) id: vec3<u32>) {
    let h = id.x;
    if (h >= p.n_opp) {
        return;
    }
    let n = topo[p.list_offset + id.y];
    let parent = field(n, PARENT);
    var r: f32;
    if (parent == NO_PARENT) {
        r = reach[(p.num_nodes + 1u - p.traverser) * p.stride + h];
    } else {
        r = reach[parent * p.stride + h];
        let kind = field(parent, KIND);
        if (kind == CHANCE) {
            r *= bitcast<f32>(field(parent, VALUE));
        } else if (field(parent, PLAYER) != p.traverser) {
            r *= probability(field(parent, STORE), field(parent, NUM_CHILDREN), p.n_opp, field(n, ACTION), h);
        }
    }
    reach[n * p.stride + h] = r;
}

// Floats of shared memory for `terminal`; gpu.rs sets it to the spot's longest list.
const SCAN: u32 = 3456u;
const GROUP: u32 = 256u;
var<workgroup> buf: array<f32, SCAN>;
var<workgroup> carry: array<f32, GROUP>;
var<workgroup> flag: array<u32, GROUP>;

fn is_head(heads: u32, x: u32) -> bool {
    return ((sd[heads + x / 32u] >> (x % 32u)) & 1u) == 1u;
}

// One workgroup per terminal node, the O(n) showdown of postflop-solver: prefix sums of the
// opponent's reach in strength order and per card (one segmented scan over all lists), then per
// hand beaten − beating, each minus the opponent hands that share a card (inclusion-exclusion).
@compute @workgroup_size(256)
fn terminal(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) t: u32) {
    let n = topo[p.list_offset + wg.y];
    let head = (field(n, BLOCK) * 2u + p.traverser) * 8u;
    let m = sd[head];
    let len = sd[head + 1u];
    let pairs_at = sd[head + 2u];
    let pairs_len = sd[head + 3u];
    let heads = sd[head + 4u];
    let records = sd[head + 5u];
    let row = n * p.stride;

    for (var x = t; x < len; x += GROUP) {
        buf[x] = 0.0;
    }
    workgroupBarrier();
    for (var k = t; k < pairs_len; k += GROUP) {
        buf[sd[pairs_at + 2u * k + 1u]] = reach[row + sd[pairs_at + 2u * k]];
    }
    workgroupBarrier();

    // Each thread scans a chunk; a segment start resets the sum (it holds 0).
    let size = (len + GROUP - 1u) / GROUP;
    let lo = min(t * size, len);
    let hi = min(lo + size, len);
    var acc = 0.0;
    var reset = 0u;
    for (var x = lo; x < hi; x++) {
        if (is_head(heads, x)) {
            acc = 0.0;
            reset = 1u;
        }
        acc += buf[x];
        buf[x] = acc;
    }
    carry[t] = acc;
    flag[t] = reset;
    workgroupBarrier();
    // Segmented inclusive scan of the chunk sums (Hillis-Steele).
    for (var d = 1u; d < GROUP; d <<= 1u) {
        var c = carry[t];
        var f = flag[t];
        if (t >= d) {
            if (f == 0u) {
                c += carry[t - d];
            }
            f |= flag[t - d];
        }
        workgroupBarrier();
        carry[t] = c;
        flag[t] = f;
        workgroupBarrier();
    }
    if (t > 0u) {
        let incoming = carry[t - 1u];
        for (var x = lo; x < hi; x++) {
            if (is_head(heads, x)) {
                break;
            }
            buf[x] += incoming;
        }
    }
    workgroupBarrier();

    let total = buf[m];
    let payoff = bitcast<f32>(field(n, VALUE));
    let fold = field(n, KIND) == FOLD;
    let sign = select(1.0, -1.0, fold && field(n, PLAYER) == p.traverser);
    for (var i = t; i < p.n_trav; i += GROUP) {
        let r = records + i * 9u;
        var v = 0.0;
        if (sd[r] != NO_PARENT) {
            if (fold) {
                let same = sd[r + 8u];
                let both = select(0.0, reach[row + min(same, p.n_opp - 1u)], same != NO_PARENT);
                v = sign * payoff * (total - buf[sd[r + 4u]] - buf[sd[r + 7u]] + both);
            } else {
                let win = buf[sd[r]] - buf[sd[r + 2u]] - buf[sd[r + 5u]];
                let lose = (total - buf[sd[r + 1u]]) - (buf[sd[r + 4u]] - buf[sd[r + 3u]]) - (buf[sd[r + 7u]] - buf[sd[r + 6u]]);
                v = payoff * (win - lose);
            }
        }
        cfv[row + i] = v;
    }
}

@compute @workgroup_size(64)
fn backward(@builtin(global_invocation_id) id: vec3<u32>) {
    let h = id.x;
    if (h >= p.n_trav) {
        return;
    }
    let n = topo[p.list_offset + id.y];
    let first = field(n, CHILDREN);
    let num_actions = field(n, NUM_CHILDREN);
    if (field(n, KIND) != DECISION || field(n, PLAYER) != p.traverser) {
        var v = 0.0;
        for (var a = 0u; a < num_actions; a++) {
            v += cfv[topo[first + a] * p.stride + h];
        }
        cfv[n * p.stride + h] = v;
        return;
    }

    let store = field(n, STORE);
    var s: array<f32, MAX_ACTIONS>;
    var sum = 0.0;
    for (var a = 0u; a < num_actions; a++) {
        s[a] = max(regret[store + a * p.n_trav + h], 0.0);
        sum += s[a];
    }
    var v = 0.0;
    for (var a = 0u; a < num_actions; a++) {
        s[a] = select(s[a] / sum, 1.0 / f32(num_actions), sum == 0.0);
        v += s[a] * cfv[topo[first + a] * p.stride + h];
    }
    cfv[n * p.stride + h] = v;
    for (var a = 0u; a < num_actions; a++) {
        let i = store + a * p.n_trav + h;
        let x = regret[i];
        // Sign bit, not `x >= 0.0`: postflop-solver discounts -0.0 like a negative regret.
        let coef = select(p.alpha, p.beta, (bitcast<u32>(x) >> 31u) == 1u);
        regret[i] = x * coef + cfv[topo[first + a] * p.stride + h] - v;
        strat_sum[i] = strat_sum[i] * p.gamma + s[a];
    }
}
