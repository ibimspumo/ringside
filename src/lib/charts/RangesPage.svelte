<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, onPreflopProgress } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import type { PushFoldAnte, PushFoldChart as PfChart, PushFoldGrid, PushFoldNode } from "../types";
  import type { PreflopGrid, PreflopNode, PreflopProgress, PreflopSpot, PreflopSpots } from "./preflop-types";
  import PushFoldChart from "./PushFoldChart.svelte";
  import PreflopChart from "./PreflopChart.svelte";
  import SeatTable from "./SeatTable.svelte";
  import StackAxis, { type Series } from "./StackAxis.svelte";
  import ActionBar from "./ActionBar.svelte";
  import ReadGuide from "./ReadGuide.svelte";
  import Hint from "./Hint.svelte";
  import { preview } from "../learn/preview";
  import { actionColor, actionName, actionPhrase, bb, colorWord, facing, spotLabel, stepTag } from "./preflop-labels";
  import { TERM, byPos, fromPos, pct, type SeatMark } from "./words";
  import { backToHand, chartsHandoff, type ChartTarget } from "./handoff.svelte";

  /** Spots reached less often than this are left out. */
  const MIN_REACH = 1e-4;
  const ANTE_LABEL: Record<PushFoldAnte, string> = { none: "Ohne", classic: "Ante", big_blind: "BB-Ante" };
  const SHOVE = actionColor("all_in");
  const CALL = actionColor("call");

  type Segment = "open" | "vs_open" | "vs_limp" | "vs_3bet" | "later" | "vs_shove";
  const SEGMENTS: Segment[] = ["open", "vs_open", "vs_limp", "vs_3bet", "later", "vs_shove"];
  const SEG_LABEL: Record<Segment, string> = {
    open: "Eröffnen",
    vs_open: "Gegen Open",
    vs_limp: "Gegen Limp",
    vs_3bet: "Gegen 3-Bet",
    later: "Gegen 4-Bet",
    vs_shove: "Gegen All-in",
  };
  const SEG_HINT: Record<Segment, string> = {
    open: "Alle vor dir haben gefoldet: Du entscheidest als Erster, ob du mitspielst.",
    vs_open: "Jemand vor dir hat erhöht (Open). Du kannst folden, callen oder erneut erhöhen (3-Bet).",
    vs_limp: "Jemand hat nur den Big Blind bezahlt (Limp). Du kannst erhöhen und so den Pot übernehmen.",
    vs_3bet: "Auf deine Erhöhung kam eine weitere Erhöhung (3-Bet) oder ein All-in.",
    later: "Spätere Erhöhungen: 4-Bet und mehr.",
    vs_shove: "Ein Spieler vor dir ist all-in gegangen. Du callst oder foldest.",
  };
  /** Standard names in preflop action order, while no chart is loaded yet. */
  const NAMES: Record<number, string[]> = {
    2: ["SB", "BB"],
    3: ["BTN", "SB", "BB"],
    4: ["CO", "BTN", "SB", "BB"],
    5: ["HJ", "CO", "BTN", "SB", "BB"],
    6: ["LJ", "HJ", "CO", "BTN", "SB", "BB"],
    7: ["UTG", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
    8: ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
    9: ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  };

  let pfGrid = $state<PushFoldGrid | null>(null);
  let preGrid = $state<PreflopGrid | null>(null);
  let ante = $state<PushFoldAnte>("big_blind");
  let players = $state(Number(preview("players")) || 9);
  let stack = $state(Number(preview("stack")) || 25);
  /** A typed depth off the grid; solved live. */
  let custom = $state(preview("stack") !== null && preview("custom") !== null);
  let seat = $state(Number(preview("seat")) || 0);
  let segment = $state<Segment>((preview("situation") as Segment) || "open");
  /** Chosen situation within the segment: node key (push/fold) or spot id (model). */
  let pick = $state<string | null>(null);
  /** Between 15 and 25 BB: the pure push/fold equilibrium instead of the model. */
  let pfView = $state(preview("pushfold") !== null);
  let only = $state<number | null>(null);
  let pin = $state<string | null>(preview("pin"));
  /** The hand the page was opened from, for "Zurück zur Hand". */
  let fromHand = $state<number | null>(preview("fromhand") ? Number(preview("fromhand")) : null);
  /** The hand's cards as a class, e.g. "99", for the way back. */
  let fromLabel = $state<string | null>(null);
  let pendingNode = $state<number | null>(null);

  onMount(() => {
    api.pushfoldGrid().then((g) => (pfGrid = g));
    api.preflopGrid().then((g) => (preGrid = g));
    const off = onPreflopProgress((p) => {
      if (p.ante !== ante || p.players !== players || p.stack_bb !== stack) return;
      progress = progress ? { first: progress.first, last: p } : { first: p, last: p };
    });
    return () => {
      off.then((f) => f());
    };
  });

  // A spot handed over from a hand: table, depth, seat, situation and the hand itself.
  $effect(() => {
    const t = chartsHandoff.target;
    if (!t) return;
    untrack(() => apply(t, chartsHandoff.hand));
    chartsHandoff.target = null;
  });

  function apply(t: ChartTarget, hand: number | null) {
    ante = t.ante;
    players = t.players;
    seat = t.seat;
    custom = false;
    stack = t.stack_bb;
    fromHand = hand;
    fromLabel = t.hand;
    only = null;
    if (t.kind === "pushfold") {
      pfView = t.stack_bb >= (preGrid ? Math.min(...preGrid.stacks) : 15);
      segment = t.shover === null ? "open" : "vs_shove";
      pick = t.shover === null ? "open" : `${t.shover}:${t.callers.join(",")}`;
    } else {
      pfView = false;
      pendingNode = t.node;
    }
    pin = null;
    queueMicrotask(() => (pin = t.hand));
  }

  // ── Which engine answers ─────────────────────────────────────────
  const pfStacks = $derived(pfGrid?.antes.find((a) => a.ante === ante)?.stacks ?? []);
  const pfMax = $derived(pfStacks.length ? pfStacks[pfStacks.length - 1] : 25);
  const modelFrom = $derived(preGrid ? Math.min(...preGrid.stacks) : null);
  const modelPlayers = $derived(!!preGrid && preGrid.players.includes(players));
  const overlap = $derived(modelPlayers && modelFrom !== null && stack >= modelFrom && stack <= pfMax);
  const axis = $derived(
    modelPlayers && !pfView && preGrid && modelFrom !== null
      ? [...pfStacks.filter((s) => s < modelFrom), ...preGrid.stacks]
      : pfStacks,
  );
  const engine = $derived<"pushfold" | "preflop">(
    !modelPlayers || pfView || modelFrom === null || stack < modelFrom ? "pushfold" : "preflop",
  );
  const playerRange = $derived(pfGrid ? range(pfGrid.min_players, pfGrid.max_players) : [2, 3, 4, 5, 6, 7, 8, 9]);

  function range(a: number, b: number): number[] {
    return Array.from({ length: b - a + 1 }, (_, i) => a + i);
  }
  function nearest(list: number[], v: number): number {
    return list.reduce((a, b) => (Math.abs(Math.log(b) - Math.log(v)) < Math.abs(Math.log(a) - Math.log(v)) ? b : a));
  }

  // Snap to the axis only once both grids are known; before that the axis is still partial.
  $effect(() => {
    if (pfGrid && preGrid && axis.length && !custom && !axis.includes(stack)) stack = nearest(axis, stack);
  });
  $effect(() => {
    if (pfView && !overlap) pfView = false;
  });
  $effect(() => {
    if (seat > players - 1) seat = players - 1;
  });

  function pickStack(v: number) {
    custom = !axis.includes(v);
    stack = v;
  }

  // ── Push/fold data ───────────────────────────────────────────────
  let pf = $state<PfChart | null>(null);
  let pfLoading = $state(false);
  let pfError = $state<string | null>(null);
  const pfCache = new Map<string, PfChart>();

  $effect(() => {
    if (engine !== "pushfold" || !pfGrid) return;
    const key = `${ante}|${players}|${stack}`;
    const hit = pfCache.get(key);
    if (hit) {
      pf = hit;
      pfError = null;
      return;
    }
    let alive = true;
    pfLoading = true;
    pfError = null;
    api
      .pushfoldChart(ante, players, stack)
      .then((c) => {
        pfCache.set(key, c);
        if (alive) pf = c;
      })
      .catch((e) => alive && (pfError = e instanceof Error ? e.message : String(e)))
      .finally(() => alive && (pfLoading = false));
    return () => (alive = false);
  });

  // ── Preflop model data ───────────────────────────────────────────
  let spots = $state<PreflopSpots | null>(null);
  let node = $state<PreflopNode | null>(null);
  let preLoading = $state(false);
  let preError = $state<string | null>(null);
  let progress = $state<{ first: PreflopProgress; last: PreflopProgress } | null>(null);
  const spotCache = new Map<string, PreflopSpots>();
  const nodeCache = new Map<string, PreflopNode>();

  $effect(() => {
    if (engine !== "preflop" || !preGrid) return;
    const key = `${ante}|${players}|${stack}`;
    const hit = spotCache.get(key);
    if (hit) {
      spots = hit;
      preError = null;
      return;
    }
    let alive = true;
    preLoading = true;
    preError = null;
    progress = null;
    api
      .preflopSpots(ante, players, stack)
      .then((s) => {
        spotCache.set(key, s);
        if (alive) spots = s;
      })
      .catch((e) => {
        if (alive) {
          preError = e instanceof Error ? e.message : String(e);
          spots = null;
        }
      })
      .finally(() => alive && (preLoading = false));
    return () => (alive = false);
  });

  const loading = $derived(engine === "pushfold" ? pfLoading : preLoading);
  const error = $derived(engine === "pushfold" ? pfError : preError);
  const live = $derived(!!preGrid && players <= preGrid.live_max_players);
  const onGrid = $derived(axis.includes(stack));

  // ── Situations ───────────────────────────────────────────────────
  type Item = { key: string; label: string; reach: number; color: string };

  const pfKey = (n: PushFoldNode) => (n.shover === null ? "open" : `${n.shover}:${n.callers.join(",")}`);
  const pfPos = (s: number) => pf?.nodes.find((n) => n.seat === s)?.position ?? "";

  function segmentOf(s: PreflopSpot): Segment {
    if (!s.line.length) return "open";
    const raises = s.line.filter((x) => x.kind === "raise" || x.kind === "all_in").length;
    if (raises === 0) return "vs_limp";
    if (raises === 1 && !s.line.some((x) => x.kind === "all_in")) return "vs_open";
    if (raises <= 2) return "vs_3bet";
    return "later";
  }

  const visible = $derived(spots?.nodes.filter((n) => n.reach >= MIN_REACH || !n.line.length) ?? []);
  const hidden = $derived((spots?.nodes.filter((n) => n.seat === seat).length ?? 0) - visible.filter((n) => n.seat === seat).length);

  const itemsBySeg = $derived.by(() => {
    const out = new Map<Segment, Item[]>();
    const add = (g: Segment, it: Item) => out.set(g, [...(out.get(g) ?? []), it]);
    if (engine === "pushfold") {
      for (const n of pf?.nodes.filter((n) => n.seat === seat) ?? []) {
        const label =
          n.shover === null
            ? "Alle vor dir haben gefoldet"
            : `Gegen All-in ${byPos(pfPos(n.shover))}${n.callers.length ? `, nach Call ${n.callers.map((c) => byPos(pfPos(c))).join(", ")}` : ""}`;
        add(n.shover === null ? "open" : "vs_shove", { key: pfKey(n), label, reach: n.reach, color: n.shover === null ? SHOVE : CALL });
      }
    } else {
      for (const s of visible.filter((n) => n.seat === seat).sort((a, b) => b.reach - a.reach)) {
        const last = s.line[s.line.length - 1];
        add(segmentOf(s), {
          key: String(s.id),
          label: s.line.length ? spotLabel(s) : "Alle vor dir haben gefoldet",
          reach: s.reach,
          color: last ? actionColor(last.kind) : "var(--text-3)",
        });
      }
    }
    return out;
  });
  const segments = $derived(SEGMENTS.filter((g) => itemsBySeg.has(g)));
  const seg = $derived(segments.includes(segment) ? segment : (segments[0] ?? "open"));
  const items = $derived(itemsBySeg.get(seg) ?? []);
  const item = $derived(items.find((i) => i.key === pick) ?? items[0] ?? null);

  // A handed-over model spot: its segment is known once the spots are there.
  $effect(() => {
    if (pendingNode === null || !spots) return;
    const s = spots.nodes.find((n) => n.id === pendingNode);
    if (s) {
      segment = segmentOf(s);
      pick = String(s.id);
    }
    pendingNode = null;
  });

  const pfNode = $derived(engine === "pushfold" && item ? (pf?.nodes.find((n) => n.seat === seat && pfKey(n) === item.key) ?? null) : null);
  const spot = $derived(engine === "preflop" && item ? (spots?.nodes.find((n) => String(n.id) === item.key) ?? null) : null);

  $effect(() => {
    if (!spots || !spot) {
      node = null;
      return;
    }
    const key = `${spots.ante}|${spots.players}|${spots.stack_bb}|${spot.id}`;
    const hit = nodeCache.get(key);
    if (hit) {
      node = hit;
      return;
    }
    let alive = true;
    api
      .preflopNode(spots.ante, spots.players, spots.stack_bb, spot.id)
      .then((n) => {
        nodeCache.set(key, n);
        if (alive) node = n;
      })
      .catch((e) => alive && (preError = e instanceof Error ? e.message : String(e)));
    return () => (alive = false);
  });

  $effect(() => {
    void item;
    void engine;
    only = null;
  });

  // ── The table ────────────────────────────────────────────────────
  const positions = $derived.by(() => {
    if (engine === "pushfold" && pf && pf.players === players) return range(0, players - 1).map((s) => ({ seat: s, name: pfPos(s) }));
    if (engine === "preflop" && spots && spots.players === players) return spots.positions.map((name, s) => ({ seat: s, name }));
    return (NAMES[players] ?? []).map((name, s) => ({ seat: s, name }));
  });
  const marks = $derived.by(() => {
    const m: Record<number, SeatMark> = {};
    if (pfNode) {
      if (pfNode.shover !== null) m[pfNode.shover] = { text: "All-in", color: SHOVE };
      for (const c of pfNode.callers) m[c] = { text: "Call", color: CALL };
    }
    spot?.line.forEach((st, i) => (m[st.seat] = { text: stepTag(st, spot.line.slice(0, i)), color: actionColor(st.kind) }));
    return m;
  });
  const folded = $derived.by(() => {
    const again = !!spot?.line.some((st) => st.seat === seat);
    return positions.filter((p) => p.seat !== seat && !(p.seat in marks) && (p.seat < seat || again)).map((p) => p.seat);
  });

  function pickSeat(s: number) {
    seat = s;
  }

  // ── The answer ───────────────────────────────────────────────────
  const shove = $derived(pfNode?.shover === null);
  const answer = $derived.by(() => {
    if (engine === "pushfold" && pfNode && pf) {
      const where = `${fromPos(pfNode.position)} mit ${bb(pf.stack_bb)} BB`;
      const vs = pfNode.shover === null ? "" : ` gegen All-in ${byPos(pfPos(pfNode.shover))}`;
      if (pfNode.act_pct < 0.05) return { where: `${where}${vs}`, parts: [], rest: "immer folden", share: 0 };
      return {
        where: `${where}${vs}`,
        parts: [{ color: shove ? SHOVE : CALL, text: shove ? "rote Hände all-in" : "grüne Hände callen" }],
        rest: "Rest folden",
        share: pfNode.act_pct,
      };
    }
    if (engine === "preflop" && spot && spots) {
      const vs = facing(spot);
      const where = `${fromPos(spot.position)} mit ${bb(spots.stack_bb)} BB${vs ? ` ${vs}` : ""}`;
      const plays = spot.actions.filter((a) => a.kind !== "fold" && a.pct >= 0.05).sort((a, b) => b.pct - a.pct);
      const fold = spot.actions.find((a) => a.kind === "fold");
      return {
        where,
        parts: plays.map((a) => ({ color: actionColor(a.kind), text: `${colorWord(a.kind)} Hände ${actionPhrase(a, spot.line)}` })),
        rest: plays.length ? (fold && fold.pct >= 0.05 ? "Rest folden" : "") : "immer folden",
        share: spot.actions.filter((a) => a.kind !== "fold").reduce((t, a) => t + a.pct, 0),
      };
    }
    return null;
  });
  const situation = $derived.by(() => {
    if (pfNode) {
      if (pfNode.shover === null) return "Alle vor dir haben gefoldet.";
      return pfNode.callers.length ? `Vor dir: All-in ${byPos(pfPos(pfNode.shover))}, Call ${pfNode.callers.map((c) => byPos(pfPos(c))).join(", ")}.` : "";
    }
    if (spot) return spot.line.length ? `Vorher: ${spotLabel(spot)}.` : "Alle vor dir haben gefoldet.";
    return "";
  });
  const barActions = $derived.by(() => {
    if (pfNode)
      return [
        { name: shove ? "All-in" : "Call", pct: pfNode.act_pct, color: shove ? SHOVE : CALL, fold: false },
        { name: "Fold", pct: 100 - pfNode.act_pct, color: "var(--surface-3)", fold: true },
      ];
    if (spot) return spot.actions.map((a) => ({ name: actionName(a, spot.line), pct: a.pct, color: actionColor(a.kind), fold: a.kind === "fold" }));
    return [];
  });

  // ── Open-raise share over the depth ──────────────────────────────
  let curve = $state<Series[] | null>(null);
  const curveCache = new Map<string, Series[]>();

  $effect(() => {
    if (!pfGrid || !preGrid) return;
    const useModel = modelPlayers && !pfView && modelFrom !== null;
    const pfDepths = useModel ? pfStacks.filter((s) => s < (modelFrom as number)) : pfStacks;
    const modelDepths = useModel ? preGrid.stacks : [];
    const key = `${ante}|${players}|${useModel}`;
    const hit = curveCache.get(key);
    if (hit) {
      curve = hit;
      return;
    }
    let alive = true;
    curve = null;
    const a = ante;
    const n = players;
    Promise.all([
      Promise.all(pfDepths.map((s) => api.pushfoldChart(a, n, s).then((c) => ({ s, c })))),
      Promise.all(modelDepths.map((s) => api.preflopSpots(a, n, s).then((c) => ({ s, c })))),
    ])
      .then(([pfs, models]) => {
        const by = new Map<number, Series>();
        const put = (seatNo: number, name: string, st: number, p: number) => {
          const ser = by.get(seatNo) ?? { seat: seatNo, name, points: [] };
          ser.points.push({ stack: st, pct: p });
          by.set(seatNo, ser);
        };
        for (const { s, c } of pfs) for (const nd of c.nodes.filter((x) => x.shover === null)) put(nd.seat, nd.position, s, nd.act_pct);
        for (const { s, c } of models)
          for (const sp of c.nodes.filter((x) => !x.line.length))
            put(
              sp.seat,
              sp.position,
              s,
              sp.actions.filter((x) => x.kind === "raise" || x.kind === "all_in").reduce((t, x) => t + x.pct, 0),
            );
        const out = [...by.values()].map((ser) => ({ ...ser, points: ser.points.sort((x, y) => x.stack - y.stack) })).sort((x, y) => x.seat - y.seat);
        curveCache.set(key, out);
        if (alive) curve = out;
      })
      .catch(() => alive && (curve = []));
    return () => (alive = false);
  });

  const mine = $derived(curve?.find((s) => s.seat === seat)?.points.find((p) => p.stack === stack)?.pct ?? null);
  const myName = $derived(positions.find((p) => p.seat === seat)?.name ?? "");

  // ── Live solve ───────────────────────────────────────────────────
  const done = $derived.by(() => {
    if (!progress) return 0;
    const { first, last } = progress;
    const span = Math.log(first.exploitability / last.target_exploitability);
    const byExpl = span > 0 ? Math.log(first.exploitability / last.exploitability) / span : 0;
    return Math.min(1, Math.max(byExpl, last.iterations / last.max_iterations, 0));
  });

  const anteText = (s: { ante_bb: number; bb_ante_bb: number }) =>
    s.ante_bb > 0 ? `Ante ${bb(s.ante_bb)} BB je Spieler` : s.bb_ante_bb > 0 ? `BB-Ante ${bb(s.bb_ante_bb)} BB, zahlt der Big Blind` : "Keine Ante, nur Blinds";
  const anteInfo = $derived(pfGrid?.antes.find((a) => a.ante === ante) ?? null);

  const sizesText = $derived.by(() => {
    if (!spots) return "";
    const z = spots.sizes;
    return `Open ${bb(z.open_bb)} BB (SB ${bb(z.sb_open_bb)} BB), Raise gegen SB-Limp ${bb(z.iso_bb)} BB, 3-Bet ${bb(z.three_bet_ip)}× in Position / ${bb(z.three_bet_oop)}× ohne Position (+${bb(z.squeeze_per_caller)}× je Caller), 4-Bet ${bb(z.four_bet)}×`;
  });
</script>

<ClientLock feature="Die Seite „Ranges nach Stack“">
  <div class="page ranges-page">
    {#if fromHand !== null}
      <!-- Stays in view while the page scrolls, without taking room. -->
      <div class="back-bar">
        <button class="btn" onclick={() => backToHand()}>
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"><path d="M14.5 6l-6 6 6 6" /></svg>
          Zurück zur Hand{fromLabel ? ` · ${fromLabel}` : ""}
        </button>
      </div>
    {/if}
    <div class="intro">
      <p>Welche Hände du vor dem Flop spielst, hängt vor allem an deinem Stack.</p>
    </div>

    {#if pfGrid && preGrid}
      <div class="body">
        <section class="panel axis-panel" aria-label="Stacktiefe">
          <StackAxis
            id="stack-axis"
            stacks={axis}
            value={stack}
            onPick={pickStack}
            series={curve}
            {seat}
            onPickSeat={pickSeat}
            modelFrom={modelPlayers && !pfView ? modelFrom : null}
            custom={engine === "pushfold" || live
              ? { digits: engine === "pushfold" ? 2 : 1, example: engine === "pushfold" ? "13,5" : "35", range: engine === "preflop" ? `${bb(preGrid.live_min_stack_bb)} bis ${bb(preGrid.live_max_stack_bb)} BB.` : null }
              : null}
          />
          {#if mine !== null}
            <p class="pfr">
              {#if seg === "open"}
                <b>{fromPos(myName)} mit {bb(stack)} BB eröffnest du etwa {pct(mine, 0)} % deiner Hände</b>
                {engine === "pushfold" ? "– jedes Mal all-in." : "– meist mit einem Raise."}
                <span class="faint">Je kürzer der Stack, desto öfter geht es direkt all-in.</span>
              {:else}
                Zum Vergleich: Wenn alle vor dir folden, eröffnest du {fromPos(myName).replace(/^\S+/, (w) => w.toLowerCase())} mit {bb(stack)} BB etwa
                {pct(mine, 0)} % deiner Hände. <span class="faint">Die Range unten gilt für „{SEG_LABEL[seg]}“.</span>
              {/if}
            </p>
          {/if}
        </section>

        <section class="panel setup" aria-label="Platz und Situation">
          <SeatTable {positions} {seat} onPick={pickSeat} {marks} {folded}>
            {#snippet middle()}
              <div class="stepper" role="group" aria-label="Spieler am Tisch">
                <button type="button" aria-label="Ein Spieler weniger" disabled={players <= playerRange[0]} onclick={() => (players -= 1)}>−</button>
                <span class="num">{players} Spieler</span>
                <button type="button" aria-label="Ein Spieler mehr" disabled={players >= playerRange[playerRange.length - 1]} onclick={() => (players += 1)}>+</button>
              </div>
            {/snippet}
          </SeatTable>
          <div class="rail">
            <span class="lbl"><Hint text={ante === "big_blind" ? TERM.bbAnte : TERM.ante}>Ante</Hint></span>
            <div class="seg" role="group" aria-label="Ante-Struktur">
              {#each pfGrid.antes as a (a.ante)}
                <button class:on={ante === a.ante} title={anteText(a)} onclick={() => (ante = a.ante)}>{ANTE_LABEL[a.ante]}</button>
              {/each}
            </div>
          </div>
          {#if !modelPlayers}
            <p class="only-pf">Mit {players} Spielern gibt es nur Push/Fold bis {bb(pfMax)} BB; das Modell mit Raises kennt {preGrid.players.join(", ")} Spieler.</p>
          {/if}

          {#if segments.length}
            <div class="situ">
              <span class="lbl">Situation</span>
              <div class="segs" role="tablist" aria-label="Situation">
                {#each segments as g (g)}
                  <button type="button" role="tab" aria-selected={seg === g} class:on={seg === g} title={SEG_HINT[g]} onclick={() => ((segment = g), (pick = null))}>
                    {SEG_LABEL[g]}
                  </button>
                {/each}
              </div>
              <p class="seg-hint">{SEG_HINT[seg]}</p>
              {#if items.length > 1}
                <span class="lbl">Genau: <span class="faint">die Zahl sagt, wie oft die Situation vorkommt</span></span>
                <div class="spots" role="radiogroup" aria-label="Genaue Situation">
                  {#each items as it (it.key)}
                    {@const on = item?.key === it.key}
                    <button type="button" role="radio" aria-checked={on} class="spot" class:on onclick={() => (pick = it.key)}>
                      <i class="sw" style="background:{it.color}"></i>
                      <span>{it.label}</span>
                      <small class="num" title="So oft kommt diese Situation an deinem Platz vor">{pct(it.reach * 100, it.reach < 0.01 ? 2 : 1)} %</small>
                    </button>
                  {/each}
                </div>
              {/if}
              {#if engine === "preflop" && hidden > 0}
                <p class="hint-line">{hidden} sehr seltene Situationen dieser Position sind ausgeblendet.</p>
              {/if}
            </div>
          {/if}
        </section>

        <section class="panel chart-panel" aria-busy={loading}>
          <header class="answer" class:dim={loading && !!answer}>
            <div class="answer-top">
              {#if engine === "pushfold"}
                <span class="badge eq"><Hint text={TERM.equilibrium}>Gleichgewicht</Hint></span>
              {:else}
                <span class="badge model"><Hint text={TERM.model}>Modell</Hint></span>
              {/if}
              {#if overlap}
                <div class="seg view" role="group" aria-label="Rechnung">
                  <button class:on={!pfView} onclick={() => (pfView = false)}>Mit Raise</button>
                  <button class:on={pfView} onclick={() => (pfView = true)}>Nur All-in</button>
                </div>
              {/if}
              {#if answer}<span class="share num">{pct(answer.share)} %<small>{engine === "pushfold" && shove ? "der Hände" : "der Hände, die hier ankommen"}</small></span>{/if}
            </div>
            {#if answer}
              <h2>
                {answer.where}:
                {#each answer.parts as p, i (i)}{i ? ", " : ""}<span class="say"><i class="sw" style="background:{p.color}"></i>{p.text}</span>{/each}{answer.parts.length && answer.rest ? ", " : ""}{answer.rest}.
              </h2>
              {#if situation}<p>{situation}</p>{/if}
            {:else if error}
              <h2>Für {bb(stack)} BB gibt es gerade kein Ergebnis.</h2>
              <p>Wähl einen anderen Stack oder geh zurück zur nächsten fertigen Tabelle.</p>
            {:else if loading}
              <h2>{bb(stack)} BB werden gerade gerechnet.</h2>
              <p>Sobald es fertig ist, steht hier die Antwort für deinen Platz.</p>
            {:else}
              <h2>Für diesen Platz gibt es hier keine Entscheidung.</h2>
            {/if}
            {#if loading && !onGrid && !error}
              <div class="live" role="status">
                {#if engine === "preflop"}
                  <div class="bar" role="progressbar" aria-label="Fortschritt der Live-Berechnung" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(done * 100)}>
                    <i style="width:{(done * 100).toFixed(1)}%"></i>
                  </div>
                  <div class="live-row">
                    <p>
                      <b class="num">{Math.round(done * 100)} %</b> · Live gerechnet; auf einem Laptop mit 4 Kernen
                      {players > 6 ? "etwa 4 bis 8 Minuten" : "etwa 2 bis 3 Minuten"}.
                      {#if progress}
                        <span class="faint num"
                          >Durchgang {progress.last.iterations}, <Hint text={TERM.exploit}>ausnutzbar</Hint>
                          {pct(progress.last.exploitability, 4)} BB (Ziel {pct(progress.last.target_exploitability, 4)} BB)</span
                        >
                      {/if}
                    </p>
                    <button class="btn" onclick={() => api.preflopCancel()}>Abbrechen</button>
                  </div>
                {:else}
                  <div class="live-row">
                    <span class="spin" aria-hidden="true"></span>
                    <p>Für {bb(stack)} BB gibt es keine fertige Tabelle; das dauert einige Sekunden.</p>
                  </div>
                {/if}
              </div>
            {/if}
            {#if barActions.length}<ActionBar actions={barActions} selected={only} onSelect={(i) => (only = i)} />{/if}
          </header>
          <div class="chart-wrap">
            <div class:stale={loading}>
              {#if pfNode}
                <PushFoldChart node={pfNode} action={shove ? "shove" : "call"} {pin} {only} />
              {:else if spot && node && node.id === spot.id}
                <PreflopChart {node} actions={spot.actions} line={spot.line} {pin} {only} />
              {:else}
                <div class="placeholder" aria-hidden="true"></div>
              {/if}
            </div>
            {#if error}
              <div class="over" role="alert">
                <p class="neg">{error}</p>
                {#if axis.length}<button class="btn" onclick={() => pickStack(nearest(axis, stack))}>Zurück zu {bb(nearest(axis, stack))} BB</button>{/if}
              </div>
            {:else if onGrid && (loading || (!pfNode && !(spot && node && node.id === spot.id) && !!item))}
              <div class="over quiet" role="status"><span class="spin" aria-hidden="true"></span><p>Wird geladen …</p></div>
            {/if}
          </div>
        </section>

        <aside class="side">
          <ReadGuide
            fill={engine === "pushfold"
              ? "Teilweise gefüllte Felder spielst du gemischt; die Zahl unten rechts ist der Anteil in Prozent."
              : "Jedes Feld ist von links nach Anteil der Aktionen gefüllt, Fold bleibt leer. Die Zahl unten rechts nennt bei gemischten Händen die häufigste Aktion in Prozent."}
          />
          {#if engine === "pushfold" && pf && pfNode}
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Dieser Spot</h2></div>
              <dl>
                <dt>Tisch</dt>
                <dd>{pf.players} Spieler, alle mit {bb(pf.stack_bb)} <Hint text={TERM.bb}>BB</Hint></dd>
                <dt>Antes</dt>
                <dd>{anteInfo ? anteText(anteInfo) : ""}</dd>
                <dt><Hint text={TERM.reach}>Kommt vor</Hint></dt>
                <dd class="num">in {pct(pfNode.reach * 100)} % der Hände</dd>
                <dt>Quelle</dt>
                <dd>{#if pf.precomputed}<Hint text={TERM.precomputed}>Vorgerechnet</Hint>{:else}<Hint text={TERM.live}>Gerade live berechnet</Hint>{/if}</dd>
              </dl>
            </section>
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Gleichgewicht, reines Push/Fold</h2></div>
              <p class="note">Jeder geht all-in oder foldet. Bewertet in Chips, <Hint text={TERM.icm}>ohne Turnier-Auszahlungen (ICM)</Hint>.</p>
              <details>
                <summary>Genauer: was angenommen wird</summary>
                <p class="note">
                  Limps und kleine Raises gibt es nicht. Höchstens drei Spieler gehen gemeinsam all-in, danach folden alle anderen. Alle
                  Spieler haben denselben Stack.
                </p>
              </details>
              {#if !pf.converged}
                <p class="note warn">
                  Nicht ganz im Gleichgewicht: Ein Spieler könnte noch {pct(pf.exploitability_bb, 3)} BB je Hand gewinnen.
                </p>
              {/if}
            </section>
          {:else if engine === "preflop" && spots && spot}
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Dieser Spot</h2></div>
              <dl>
                <dt>Tisch</dt>
                <dd>{spots.players} Spieler, alle mit {bb(spots.stack_bb)} <Hint text={TERM.bb}>BB</Hint></dd>
                <dt>Antes</dt>
                <dd>{anteText(spots)}</dd>
                <dt><Hint text={TERM.reach}>Kommt vor</Hint></dt>
                <dd class="num">in {pct(spot.reach * 100, 2)} % der Hände</dd>
                <dt>Quelle</dt>
                <dd>{#if spots.precomputed}<Hint text={TERM.precomputed}>Vorgerechnet</Hint>{:else}<Hint text={TERM.live}>Gerade live berechnet</Hint>{/if}</dd>
                <dt><Hint text={TERM.exploit}>Ausnutzbar</Hint></dt>
                <dd class="num">{pct(spots.exploitability_bb, 4)} BB je Hand (alle Spieler zusammen, im Modell)</dd>
              </dl>
            </section>
            <section class="panel facts">
              <div class="panel-head"><h2 class="panel-title">Modell, kein exaktes Gleichgewicht</h2></div>
              <p class="note">
                Preflop wird gelöst, das Spiel nach dem Flop nur geschätzt. Ringside nennt das bewusst nicht „GTO“.
              </p>
              {#if !spots.converged}
                <p class="note warn">Nicht ganz konvergiert: Die Ausnutzbarkeit liegt über dem Ziel von {pct(preGrid.target_exploitability_bb, 4)} BB.</p>
              {/if}
              <details>
                <summary>Genauer: wie das Modell rechnet</summary>
                <p class="note">
                  Jeder bekommt seinen Anteil am Pot nach Equity, gewichtet mit Position, Initiative, Spielbarkeit der Hand und Stacktiefe.
                  Feste Einsatzgrößen, höchstens ein Caller vor dem Big Blind.
                </p>
                <p class="note">
                  <b>Größen:</b>
                  {sizesText}; 5-Bet nur All-in. Ein Raise, der mindestens {pct(spots.sizes.commit * 100, 0)} % des Stacks kostet, wird zum All-in.
                </p>
                <p class="note">
                  <b>Nach dem Flop:</b> Jeder Spieler erhält vom Pot den Anteil Equity<sup>{bb(spots.realization.sharpness)}</sup> × Gewicht, geteilt
                  durch die Summe aller Spieler. Der Exponent lässt starke Hände mehr und schwache weniger realisieren. Das Gewicht wächst mit der
                  Position (×{bb(spots.realization.position)} je Spieler, der vorher handelt), für den letzten Aggressor (×{bb(spots.realization.initiative)})
                  und mit der Spielbarkeit (suited ×{bb(spots.realization.suited)}, offsuit ×{bb(spots.realization.offsuit)}, verbunden
                  +{pct(spots.realization.connected * 100, 0)} %, große Lücke −{pct(spots.realization.gapped * 100, 0)} %). Je kleiner das
                  Stack-zu-Pot-Verhältnis, desto mehr zählt nur die reine Equity; All-in-Pots werden exakt abgerechnet.
                </p>
                <p class="note">
                  <b>Vereinfachungen:</b> Nach einer 3-Bet spielt nur der erste, der weitermacht; nach einem All-in nur der erste Caller. Höchstens
                  drei Spieler sehen einen Flop. Chip-EV <Hint text={TERM.icm}>ohne ICM</Hint>, Kartenentfernung paarweise genähert.
                </p>
                {#if spots.realized.length}
                  <table class="eqr">
                    <caption>Realisierte Equity im Modell (häufigste Flops)</caption>
                    <thead><tr><th>Pot</th><th>Spieler</th><th>Equity</th><th>Realisiert</th></tr></thead>
                    <tbody>
                      {#each spots.realized.slice(0, 5) as r, i (i)}
                        <tr>
                          <td class="num">{bb(r.pot_bb)} BB</td>
                          <td>{r.positions.join(" / ")}</td>
                          <td class="num">{r.equity.map((e) => pct(e * 100, 0)).join(" / ")} %</td>
                          <td class="num">{r.realization.map((e) => pct(e * 100, 0)).join(" / ")} %</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                  <p class="note">Erster Spieler handelt nach dem Flop zuerst. Veröffentlichte Solver-Werte: BTN gegen BB etwa 115 % / 80 %.</p>
                {/if}
              </details>
            </section>
          {/if}
        </aside>
      </div>
    {/if}
  </div>
</ClientLock>

<style>
  /* The grid is the point of the page: on wide screens it may grow past the usual page width. */
  .ranges-page {
    max-width: 1600px;
  }
  .back-bar {
    position: sticky;
    top: 12px;
    z-index: 8;
    height: 0;
    display: flex;
    justify-content: flex-end;
  }
  .back-bar .btn {
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
  }
  .back-bar svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .intro {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 16px;
  }
  .intro p {
    margin: 0;
    color: var(--text-2);
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 880px) minmax(360px, 1fr);
    grid-template-areas:
      "axis setup"
      "chart setup"
      "chart side";
    grid-template-rows: auto auto 1fr;
    gap: 16px;
    align-items: start;
  }
  .axis-panel {
    grid-area: axis;
    padding: 14px 20px 14px;
  }
  .pfr {
    margin: 8px 0 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .pfr b {
    color: var(--text);
    font-weight: 600;
  }
  .setup {
    grid-area: setup;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px 18px 18px;
  }
  .setup :global(.seat-table) {
    width: 100%;
    max-width: 380px;
    align-self: center;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 2px;
    width: 96px;
    height: 24px;
    border-radius: 99px;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    box-shadow: inset 0 0 0 1px var(--line-2);
    font-size: 11.5px;
    color: var(--text-2);
  }
  .stepper span {
    flex: 1;
    text-align: center;
    white-space: nowrap;
  }
  .stepper button {
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }
  .stepper button:hover:not(:disabled) {
    background: var(--surface-3);
  }
  .stepper button:disabled {
    color: var(--text-3);
    cursor: default;
  }
  .stepper button:focus-visible {
    outline: 2px solid var(--focus);
  }
  .rail {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .lbl {
    font-size: 12px;
    color: var(--text-2);
  }
  .only-pf {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--amber) 8%, var(--surface));
    font-size: 12.5px;
    color: var(--text-2);
  }
  .situ {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .segs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 3px;
    border-radius: 9px;
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .segs button {
    flex: 1 1 auto;
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .segs button:hover {
    color: var(--text);
  }
  .segs button.on {
    background: var(--surface-3);
    color: var(--text);
    font-weight: 600;
  }
  .segs button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }
  .seg-hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
    line-height: 1.45;
  }
  .spots {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .spot {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
    padding: 4px 12px 4px 10px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .spot:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .spot.on {
    color: var(--text);
    border-color: var(--text-2);
    background: var(--surface-3);
  }
  .spot small {
    color: var(--text-3);
    font-size: 11.5px;
  }
  .spot:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .hint-line {
    margin: 0;
    font-size: 12px;
    color: var(--text-3);
  }

  .chart-panel {
    grid-area: chart;
    container-type: inline-size;
  }
  .answer {
    padding: 16px 20px 0;
  }
  .answer.dim h2,
  .answer.dim > p,
  .answer.dim :global(.action-bar) {
    opacity: 0.35;
    transition: opacity var(--dur) var(--ease-out);
  }
  .answer-top {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 8px;
  }
  .badge {
    padding: 1px 8px;
    border-radius: 99px;
    font-size: 11.5px;
    font-weight: 600;
    line-height: 20px;
  }
  .badge.eq {
    color: var(--pos);
    background: color-mix(in srgb, var(--pos) 13%, transparent);
  }
  .badge.model {
    color: var(--amber);
    background: color-mix(in srgb, var(--amber) 13%, transparent);
  }
  .view button {
    height: 22px;
    font-size: 12px;
  }
  .share {
    margin-left: auto;
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    font-size: 20px;
    font-weight: 600;
  }
  .share small {
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 400;
    color: var(--text-2);
  }
  .answer h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    line-height: 1.4;
    text-wrap: balance;
  }
  .say {
    white-space: nowrap;
  }
  .say .sw {
    display: inline-block;
    width: 11px;
    height: 11px;
    margin: 0 6px 0 2px;
  }
  .answer p {
    margin: 4px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .answer :global(.action-bar) {
    margin-top: 14px;
  }
  .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .chart-wrap {
    position: relative;
    padding: 12px 20px 20px;
  }
  .stale {
    opacity: 0.3;
    transition: opacity var(--dur) var(--ease-out);
  }
  .placeholder {
    aspect-ratio: 1;
    border-radius: var(--r-sm);
    background:
      repeating-linear-gradient(90deg, var(--surface-2) 0 calc(100% / 13 - 2px), transparent 0 calc(100% / 13)),
      var(--surface);
    opacity: 0.6;
  }
  /* Near the top of the grid, so it stays in view on short windows. */
  .over {
    position: absolute;
    left: 50%;
    top: 56px;
    transform: translateX(-50%);
    width: min(380px, calc(100% - 40px));
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 18px 20px;
    border-radius: var(--r);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
    text-align: center;
    font-size: 13px;
    animation: pop var(--dur) var(--ease-out);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translate(-50%, 6px);
    }
  }
  .over.quiet {
    flex-direction: row;
    justify-content: center;
    width: auto;
    padding: 10px 16px;
  }
  .over p {
    margin: 0;
    color: var(--text-2);
  }
  .live {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
    padding: 12px 14px;
    border-radius: var(--r);
    border: 1px solid var(--line-2);
    background: var(--surface-2);
    animation: pop-live var(--dur) var(--ease-out);
  }
  @keyframes pop-live {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .live-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .live-row p {
    flex: 1;
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .live-row b {
    color: var(--text);
  }
  .bar {
    align-self: stretch;
    height: 6px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--text);
  }
  .spin {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid var(--surface-3);
    border-top-color: var(--text);
    animation: spin 0.9s var(--ease-in-out) infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .side {
    grid-area: side;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .facts {
    padding-bottom: 16px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    margin: 12px 18px 0;
    font-size: 13px;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .note {
    margin: 10px 18px 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
  }
  .note b {
    color: var(--text);
    font-weight: 600;
  }
  .note.warn {
    color: var(--amber);
  }
  details {
    margin: 12px 18px 0;
    border-top: 1px solid var(--line);
    padding-top: 10px;
  }
  summary {
    font-size: 12.5px;
    color: var(--text-2);
    cursor: pointer;
  }
  summary:hover {
    color: var(--text);
  }
  details .note {
    margin-inline: 0;
  }
  .eqr {
    margin: 12px 0 0;
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .eqr caption {
    text-align: left;
    color: var(--text-2);
    margin-bottom: 6px;
  }
  .eqr th {
    text-align: left;
    font-weight: 500;
    color: var(--text-3);
    padding: 2px 8px 4px 0;
  }
  .eqr td {
    padding: 3px 8px 3px 0;
    border-top: 1px solid var(--line);
  }

  @container (max-width: 980px) {
    .body {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto;
      grid-template-areas: "axis" "setup" "chart" "side";
    }
    /* Narrow window: table on the left, situations on the right, so the answer stays high. */
    .setup {
      display: grid;
      grid-template-columns: 340px minmax(0, 1fr);
      grid-template-areas: "table situ" "table rail" "note note";
      gap: 10px 22px;
      align-items: start;
    }
    .setup :global(.seat-table) {
      grid-area: table;
    }
    .setup :global(.seat-table .cap) {
      display: none;
    }
    .rail {
      grid-area: rail;
      justify-content: flex-start;
    }
    .intro p {
      display: none;
    }
    .only-pf {
      grid-area: note;
    }
    .situ {
      grid-area: situ;
      padding-top: 0;
      border-top: 0;
    }
  }
</style>
