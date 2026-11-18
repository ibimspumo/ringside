// Measures the tables of the browser demo (practice, solver, learning area) at many sizes:
// seats, action badges, chips, dealer button and the middle (board and pot) must not overlap,
// chips stay on the felt, the table is mirror-symmetric and "you" sit bottom centre.
//
//   pnpm vite --port 5317 &  chromedriver --port=9617 &
//   E2E_URL=http://127.0.0.1:5317 E2E_WEBDRIVER=http://127.0.0.1:9617 node e2e/table-geometry.mjs
//
// E2E_SHOTS=<folder> also saves a screenshot per player count.

import { writeFileSync } from "node:fs";
import { Session } from "./webdriver.mjs";

const URL = process.env.E2E_URL ?? "http://127.0.0.1:5317";
const DRIVER = process.env.E2E_WEBDRIVER ?? "http://127.0.0.1:9617";
const SHOTS = process.env.E2E_SHOTS;

const MEASURE = `
  const box = (el) => { const r = el.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom, x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 }; };
  const parts = [...document.querySelectorAll("[data-part]")].map((el) => ({ kind: el.dataset.part, seat: el.dataset.seat === undefined ? null : Number(el.dataset.seat), ...box(el) }));
  const cloth = document.querySelector(".cloth");
  return cloth ? { parts, cloth: box(cloth) } : null;
`;

const overlap = (a, b, pad = 1) => a.l < b.r - pad && b.l < a.r - pad && a.t < b.b - pad && b.t < a.b - pad;
const insideEllipse = (p, e) => {
  const cx = (e.l + e.r) / 2;
  const cy = (e.t + e.b) / 2;
  const rx = (e.r - e.l) / 2;
  const ry = (e.b - e.t) / 2;
  return [
    [p.l, p.t],
    [p.r, p.t],
    [p.l, p.b],
    [p.r, p.b],
  ].every(([x, y]) => ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0001);
};

/** Practice table, solver table (Aufbau, Durchspielen, Üben) and the learning area's seat ring. */
const VIEWS = [
  { kind: "play", widths: [560, 640, 760, 900, 1040, 1180], players: [2, 3, 4, 5, 6, 7, 8, 9] },
  { kind: "solver", widths: [640, 900, 1180], players: [6, 8, 9] },
  { kind: "ring", widths: [420, 520], players: [6, 8, 9] },
];

const s = await Session.start(DRIVER, { browserName: "chrome", "goog:chromeOptions": { args: ["--headless=new", "--window-size=2500,1100", "--force-device-scale-factor=1"] } });
const problems = [];
let checked = 0;
try {
  for (const view of VIEWS) {
    for (const n of view.players) {
      for (const hero of view.kind === "ring" ? [0] : [0, n - 1]) {
        for (const w of view.widths) {
          for (const height of [800, 1100]) {
            await s.cmd("POST", "/window/rect", { width: 2500, height });
            await s.navigate(`${URL}/?view=table-lab&kind=${view.kind}&n=${n}&hero=${hero}&w=${w}`);
            let m = null;
            for (let i = 0; i < 40 && !m?.parts?.length; i++) {
              m = await s.exec(`try { ${MEASURE} } catch { return null }`).catch(() => null);
              if (!m?.parts?.length) await new Promise((r) => setTimeout(r, 100));
            }
            const where = `${view.kind} n=${n} hero=${hero} w=${w} h=${height}`;
            if (!m?.parts?.length) throw new Error(`${where}: no table`);
            checked++;
            const items = m.parts;
            const of = (kind) => items.filter((x) => x.kind === kind);
            // The others' badges sit on the edge of their own seat block by design.
            const onEdge = (a, b) => a.seat !== null && a.seat === b.seat && a.seat !== hero && [a.kind, b.kind].sort().join() === "badge,seat";
            for (let i = 0; i < items.length; i++)
              for (let j = i + 1; j < items.length; j++)
                if (!onEdge(items[i], items[j]) && overlap(items[i], items[j])) problems.push(`${where}: ${items[i].kind}#${items[i].seat} overlaps ${items[j].kind}#${items[j].seat}`);
            for (const x of of("bet")) if (!insideEllipse(x, m.cloth)) problems.push(`${where}: bet#${x.seat} leaves the felt`);
            const cx = (m.cloth.l + m.cloth.r) / 2;
            const place = (seat) => (seat - hero + n) % n;
            for (const kind of view.kind === "play" ? ["seat", "badge", "bet"] : ["seat"]) {
              const by = new Map(of(kind).map((x) => [place(x.seat), x]));
              for (const [k, a] of by) {
                const b = by.get((n - k) % n);
                if (!b) continue;
                if (Math.abs(a.x - cx + (b.x - cx)) > 1.5 || Math.abs(a.y - b.y) > 1.5) problems.push(`${where}: ${kind} of places ${k} and ${(n - k) % n} are not mirrored`);
              }
            }
            const seats = of("seat");
            const me = seats.find((x) => x.seat === hero);
            if (!me || Math.abs(me.x - cx) > 1 || seats.some((x) => x.y > me.y + 0.5)) problems.push(`${where}: you do not sit bottom centre`);
            // Heads-up the opponent faces you across the table.
            const other = seats.find((x) => x.seat !== hero);
            if (n === 2 && (!other || Math.abs(other.x - cx) > 1 || other.y >= m.cloth.y)) problems.push(`${where}: the heads-up opponent does not sit top centre`);
            if (SHOTS && view.kind === "play" && w === 900 && height === 1100 && hero === 0 && [2, 3, 4, 5, 6, 8, 9].includes(n)) {
              writeFileSync(`${SHOTS}/tisch-${n}max-alle-einsaetze.png`, Buffer.from(await s.screenshot(), "base64"));
            }
            if (SHOTS && view.kind !== "play" && w === view.widths[1] && height === 1100 && hero === 0) {
              writeFileSync(`${SHOTS}/${view.kind}-${n}max-du-unten.png`, Buffer.from(await s.screenshot(), "base64"));
            }
          }
        }
      }
    }
  }
} finally {
  await s.quit();
}
console.log(`${checked} layouts measured, ${problems.length} problems`);
for (const p of process.env.E2E_ALL ? problems : problems.slice(0, 40)) console.log(`  ${p}`);
process.exit(problems.length ? 1 : 0);
