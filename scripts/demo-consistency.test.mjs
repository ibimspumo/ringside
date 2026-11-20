// The browser demo tells one story: what one page shows must not contradict another (totals,
// places, dates, preflop shares). Loads the demo modules through Vite, as the app does.
import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";

const root = fileURLToPath(new URL("..", import.meta.url));
globalThis.window = { location: { search: "" } };

let vite;
let m;
before(async () => {
  vite = await createServer({ root, configFile: false, logLevel: "silent", appType: "custom", server: { middlewareMode: true, hmr: false, ws: false }, optimizeDeps: { noDiscovery: true, include: [] } });
  const load = (path) => vite.ssrLoadModule(path);
  m = {
    world: await load("/src/lib/demo-world.ts"),
    demo: await load("/src/lib/demo.ts"),
    session: await load("/src/lib/session/demo.ts"),
    skill: await load("/src/lib/skill/demo.ts"),
    coach: await load("/src/lib/coach/demo.ts"),
    report: await load("/src/lib/report/demo.ts"),
    analysis: await load("/src/lib/analysis/demo.ts"),
    next: await load("/src/lib/next/demo.ts"),
    preflop: await load("/src/lib/charts/preflop-demo.ts"),
    pushfold: await load("/src/lib/charts/demo.ts"),
    share: await load("/src/lib/charts/demo-share.ts"),
  };
});
after(() => vite?.close());

const ALL = { game: null, since: null, tournament_id: null };
const sum = (xs) => xs.reduce((a, b) => a + b, 0);
const round1 = (x) => Math.round(x * 10) / 10;
const within = (at, from, to) => at >= from && at <= to;

test("tournaments: newest first, nothing after now, results add up", () => {
  const { rows } = m.demo.demoTournaments();
  const { NOW } = m.world;
  for (let i = 1; i < rows.length; i++) assert.ok(rows[i - 1].started_at >= rows[i].started_at, `${rows[i - 1].started_at} before ${rows[i].started_at}`);
  for (const r of rows) {
    const t = m.world.worldTournament(r.id);
    assert.ok(t.ended_at <= NOW, `${r.id} ends ${t.ended_at}, after now`);
    if (r.place !== null && r.entrants !== null) assert.ok(r.place <= r.entrants, `${r.id}: place ${r.place} of ${r.entrants}`);
    if (r.profit !== null) assert.equal(r.profit, r.prize + r.bounties - r.cost, `${r.id}: profit`);
    if ((r.prize ?? 0) > 0) assert.ok(r.profit > -r.cost, `${r.id}: a cash`);
  }
  assert.equal(rows.filter((r) => r.profit === null).length, 1, "one tournament still running");
});

test("sessions: one list for the start page and the statistics, made of the tournaments", () => {
  const spans = m.session.demoSession(null).sessions;
  const stats = m.demo.demoReports(ALL).sessions;
  const { rows } = m.demo.demoTournaments();
  assert.deepEqual(
    spans.map((s) => [s.start, s.end, s.hands]),
    stats.map((s) => [s.start, s.end, s.hands]),
  );
  for (let i = 0; i < spans.length; i++) {
    const s = spans[i];
    if (i > 0) assert.ok(spans[i - 1].start > s.end, `sessions overlap: ${s.start}`);
    assert.ok(s.end <= m.world.NOW, `${s.start} ends after now`);
    const ts = s.tournaments.map((id) => rows.find((r) => r.id === id));
    assert.ok(ts.every(Boolean), `${s.start}: unknown tournament`);
    for (const t of ts) assert.ok(within(t.started_at, s.start, s.end), `${t.id} starts ${t.started_at} outside ${s.start}–${s.end}`);
    // A session ends after 30 minutes without a hand.
    let reach = s.start;
    for (const t of [...ts].sort((a, b) => a.started_at.localeCompare(b.started_at))) {
      assert.ok(m.world.minutesBetween(reach, t.started_at) <= 30, `${s.start}: a pause before ${t.started_at}`);
      const end = m.world.worldTournament(t.id).ended_at;
      if (end > reach) reach = end;
    }
    assert.equal(s.hands, sum(ts.map((t) => t.hands)) + s.cash_hands, `${s.start}: hands`);
    const usd = sum(ts.filter((t) => t.currency === "USD").map((t) => t.bounties));
    assert.equal(stats[i].bounties.USD ?? 0, usd, `${s.start}: bounties`);
  }
});

test("all hands: overview, statistics, players and analysis tell the same totals", () => {
  const d = m.demo.demoDashboard(ALL);
  const rep = m.demo.demoReports(ALL);
  const hero = m.demo.demoPlayers(ALL)[0];
  const hands = sum(rep.sessions.map((s) => s.hands));
  assert.equal(d.results.hands, hands);
  assert.equal(d.hand_count, hands);
  assert.equal(hero.stats.find((s) => s.id === "hands").value, hands);
  assert.equal(sum(rep.positions.map((p) => p.stats[0].value)), hands, "hands by position");
  assert.equal(sum(rep.starting_hands.map((p) => p.stats[0].value)), hands, "hands by starting hand");
  assert.equal(m.analysis.demoSummary().hands, hands, "analysed hands");
  const net = round1(sum(rep.sessions.map((s) => s.net_bb)));
  assert.equal(round1(d.results.net_bb), net, "overview vs sessions");
  assert.equal(round1(sum(rep.positions.map((p) => p.net_bb))), net, "positions vs sessions");
  assert.ok(Math.abs(sum(rep.starting_hands.map((p) => p.net_bb)) - net) < 0.05, "starting hands vs sessions");
  assert.ok(Math.abs(d.results.points.at(-1).net_bb - net) < 0.05, "curve ends on the total");
  const { rows } = m.demo.demoTournaments();
  assert.equal(d.results.bounties.USD, sum(rows.filter((r) => r.currency === "USD").map((r) => r.bounties)), "bounties");
});

test("one tournament: hands and result the same in list, overview, statistics, players and report", () => {
  for (const r of m.demo.demoTournaments().rows.filter((x) => x.profit !== null).slice(0, 6)) {
    const f = { game: null, since: null, tournament_id: r.id };
    const d = m.demo.demoDashboard(f);
    assert.equal(d.results.hands, r.hands, `${r.id}: overview`);
    assert.equal(sum(m.demo.demoReports(f).positions.map((p) => p.stats[0].value)), r.hands, `${r.id}: positions`);
    const players = m.demo.demoPlayers(f);
    assert.equal(players[0].stats[0].value, r.hands, `${r.id}: hero`);
    for (const p of players.slice(1)) assert.ok(p.stats[0].value <= r.hands, `${r.id}: ${p.name} has more hands than the hero`);
    const rep = m.report.demoReport(r.id);
    assert.equal(rep.hands, r.hands, `${r.id}: report`);
    assert.equal(rep.luck.net_bb, round1(d.results.net_bb), `${r.id}: report result`);
    const end = m.world.worldTournament(r.id).ended_at;
    for (const x of [...rep.worst, ...rep.best]) assert.ok(within(x.played_at, r.started_at, end) && x.n <= r.hands, `${r.id}: moment at ${x.played_at}, hand ${x.n}`);
  }
});

test("the last session: start page shows the list's tournaments and the statistics' result", () => {
  const page = m.session.demoSession(null);
  const { rows } = m.demo.demoTournaments();
  for (const t of page.tournaments) assert.deepEqual(t, rows.find((r) => r.id === t.id));
  const s = m.demo.demoReports(ALL).sessions[0];
  assert.equal(page.review.span.start, s.start);
  assert.equal(page.review.report.luck.net_bb, s.net_bb);
  for (const w of page.review.report.worst) assert.ok(within(w.played_at, s.start, s.end), `worst hand at ${w.played_at}`);
});

test("dates: hands, shown cards, coach reviews and the rating fall into their sessions, in order", () => {
  const { NOW, WORLD_SESSIONS, worldTournament, epochOf } = m.world;
  const last = WORLD_SESSIONS[0];
  const inSome = (at) => WORLD_SESSIONS.some((s) => within(at, s.start, s.end));
  for (const h of m.demo.demoHands({ ...ALL, player: null })) assert.ok(within(h.played_at, last.start, last.end), `hand ${h.id} at ${h.played_at}`);
  assert.ok(within(m.demo.demoHandById(250000000002).played_at, last.start, last.end));
  for (const h of m.demo.demoShown()) assert.ok(inSome(h.played_at), `shown hand at ${h.played_at}`);
  const gg = worldTournament(m.world.GG_TOURNAMENT);
  for (const h of m.demo.demoHands({ ...ALL, player: null, tournament_id: gg.row.id })) assert.ok(within(h.played_at, gg.row.started_at, gg.ended_at), `GG hand at ${h.played_at}`);
  const rows = new Map(m.demo.demoTournaments().rows.map((r) => [r.id, r]));
  const reports = m.coach.list(new Map());
  assert.ok(reports.length >= 2);
  for (const r of reports) {
    const t = worldTournament(r.tournament_id);
    assert.ok(r.created_at >= epochOf(t.ended_at) && r.created_at <= epochOf(NOW), `coach review of ${r.tournament_id} before its end or after now`);
    for (const sp of m.coach.get(r.tournament_id).spots) assert.ok(within(sp.played_at, t.row.started_at, t.ended_at), `coach spot at ${sp.played_at}`);
  }
  const skill = m.skill.demoSkill();
  assert.equal(skill.entries.length, [...rows.values()].filter((r) => r.profit !== null).length, "rating over every finished tournament");
  for (let i = 1; i < skill.entries.length; i++) assert.ok(skill.entries[i - 1].started_at <= skill.entries[i].started_at);
});

test("costliest decisions: the same hand, cards and cost wherever they are named", () => {
  const list = m.analysis.withWorst(m.demo.demoHands({ ...ALL, player: null }), { ...ALL, player: null });
  for (const c of m.analysis.demoSummary().costliest) {
    const row = list.find((h) => h.id === c.hand_id);
    assert.equal(c.hero_cards, row.hero_cards, `${c.hand_id}: cards`);
    assert.equal(c.position, row.hero_position, `${c.hand_id}: seat`);
    assert.equal(c.ev_loss_bb, row.worst.ev_loss_bb, `${c.hand_id}: cost`);
    const step = m.next.demoNextSteps().find((s) => s.hand_id === c.hand_id);
    if (step) assert.equal(step.total_bb, c.ev_loss_bb, `${c.hand_id}: next step`);
  }
});

test("starting-hand charts: the sentence, the grid and the table sizes give one share", async () => {
  const { combos } = m.share;
  const shareOf = (row) => (100 * row.reduce((t, f, i) => t + (combos(i) * f) / 255, 0)) / 1326;
  for (const [players, stack] of [[9, 25], [8, 40], [6, 15], [5, 100], [3, 20]]) {
    const spots = await m.preflop.demoSpots("big_blind", players, stack);
    const opens = (await m.preflop.demoTableOpens("big_blind", stack, false)).find((r) => r.players === players);
    for (const n of spots.nodes.filter((x) => !x.line.length)) {
      const said = sum(n.actions.filter((a) => a.kind !== "fold").map((a) => a.pct));
      const node = await m.preflop.demoNode("big_blind", players, stack, n.id);
      const fold = n.actions.findIndex((a) => a.kind === "fold");
      assert.ok(Math.abs(said - (100 - shareOf(node.freq[fold]))) < 0.01, `${players}p ${stack} BB ${n.position}: sentence ${said} vs grid`);
      const real = opens.pct[opens.positions.indexOf(n.position)];
      assert.ok(Math.abs(said - real) < 0.6, `${players}p ${stack} BB ${n.position}: ${said.toFixed(1)} % vs table sizes ${real} %`);
    }
  }
  for (const [players, stack] of [[9, 10], [6, 10], [4, 15], [8, 7.5]]) {
    const chart = await m.pushfold.demoChart(stack, players, "big_blind");
    const opens = (await m.preflop.demoTableOpens("big_blind", stack, true)).find((r) => r.players === players);
    for (const n of chart.nodes) {
      assert.ok(Math.abs(n.act_pct - shareOf(n.freq)) < 0.01, `push/fold ${players}p ${stack} BB ${n.position}: share vs grid`);
      if (n.shover === null) {
        const real = opens.pct[opens.positions.indexOf(n.position)];
        assert.ok(Math.abs(n.act_pct - real) < 0.6, `push/fold ${players}p ${stack} BB ${n.position}: ${n.act_pct.toFixed(1)} % vs table sizes ${real} %`);
      }
    }
  }
});
