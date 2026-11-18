<script lang="ts">
  import { usePageHead } from "../ui/page.svelte";
  import { onMount } from "svelte";
  import ClientLock from "../app/ClientLock.svelte";
  import { libraryApi, onLibraryStatus } from "./api";
  import BackgroundMode from "./BackgroundMode.svelte";
  import { cards, depthOf, firstOf, flopName, MARK_NAME, markOf, markText, type Mark, situationOf, storyOf, suit, titleOf } from "./labels";
  import { openInSolver } from "./open";
  import RunControl from "./RunControl.svelte";
  import ShareMark from "./ShareMark.svelte";
  import type { LibraryMode, LibraryOverview, LibraryScenario, WorkerStatus } from "./types";

  /** Switches to the solver page once a spot is loaded there. */
  let { onOpen }: { onOpen: () => void } = $props();

  let data = $state<LibraryOverview | null>(null);
  let worker = $state<WorkerStatus | null>(null);
  let selected = $state<string | null>(null);
  let error = $state<string | null>(null);
  let opening = $state<string | null>(null);

  async function load() {
    try {
      const o = await libraryApi.overview();
      data = o;
      worker = o.worker;
      error = null;
      if (!selected || !o.scenarios.some((s) => s.id === selected)) selected = o.scenarios[0]?.id ?? null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    load();
    const sub = onLibraryStatus((s) => {
      const before = worker;
      worker = s;
      if (!before || before.solved !== s.solved || before.fetched !== s.fetched || before.shared !== s.shared) load();
    });
    const timer = setInterval(load, 20_000);
    return () => {
      clearInterval(timer);
      sub.then((f) => f());
    };
  });

  function setMode(m: LibraryMode) {
    if (data) data.mode = m;
    libraryApi.setMode(m);
  }

  async function open(s: LibraryScenario, flop: string) {
    opening = flop;
    try {
      await openInSolver(s, flop);
      onOpen();
    } catch (e) {
      error = String(e);
    } finally {
      opening = null;
    }
  }

  const ALL_FLOPS = 22_100;
  /** Share of all possible flops that the solved flops of the subset stand for. */
  const coverage = (s: LibraryScenario) =>
    s.status.reduce((sum, st, i) => sum + (st ? (data?.flop_weights[i] ?? 0) : 0), 0) / ALL_FLOPS;
  const count = (s: LibraryScenario, what: "local" | "cloud" | null) => s.status.filter((x) => x === what).length;
  const totals = $derived.by(() => {
    const all = data?.scenarios ?? [];
    const local = all.reduce((n, s) => n + count(s, "local"), 0);
    const cloud = all.reduce((n, s) => n + count(s, "cloud"), 0);
    const spots = all.reduce((n, s) => n + s.status.length, 0);
    const started = all.filter((s) => s.status.some(Boolean)).length;
    const marks = all.flatMap((s) => s.status.flatMap((st, i) => (st === "local" ? [markOf(s.shared[i])] : [])));
    const n = (m: Mark) => marks.filter((x) => x === m).length;
    const served = n("served");
    const contributed = served + n("submitted");
    return {
      local,
      cloud,
      spots,
      done: local + cloud,
      started,
      situations: all.length,
      contributed,
      served,
      rejected: n("rejected"),
      waiting: n("only_here") + n("failed"),
    };
  });
  const stacks = $derived([...new Set((data?.scenarios ?? []).map((s) => s.stack_bb))]);
  const current = $derived(data?.scenarios.find((s) => s.id === selected) ?? null);

  type Texture = "Rainbow" | "Zweifarbig" | "Monoton" | "Gepaart" | "Drilling";
  const TEXTURE_HINT: Record<Texture, string> = {
    Rainbow: "drei verschiedene Farben, kein Flush möglich",
    Zweifarbig: "zwei Karten einer Farbe, Flush-Draws möglich",
    Monoton: "alle drei in einer Farbe",
    Gepaart: "zwei Karten gleichen Rangs",
    Drilling: "drei Karten gleichen Rangs",
  };
  function texture(f: string): Texture {
    const [a, b, c] = cards(f);
    const ranks = new Set([a[0], b[0], c[0]]).size;
    if (ranks === 1) return "Drilling";
    if (ranks === 2) return "Gepaart";
    const suits = new Set([a[1], b[1], c[1]]).size;
    return suits === 1 ? "Monoton" : suits === 2 ? "Zweifarbig" : "Rainbow";
  }
  const groups = $derived.by(() => {
    if (!data) return [];
    const out: { name: Texture; flops: { flop: string; i: number }[] }[] = [];
    data.flops.forEach((flop, i) => {
      const name = texture(flop);
      let g = out.find((x) => x.name === name);
      if (!g) out.push((g = { name, flops: [] }));
      g.flops.push({ flop, i });
    });
    return out;
  });

  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const num = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 2 });
  const mb = (b: number) =>
    b >= 1e9 ? `${(b / 1e9).toLocaleString("de-DE", { maximumFractionDigits: 1 })} GB` : `${Math.round(b / 1e6).toLocaleString("de-DE")} MB`;
  const MARKS: Mark[] = ["only_here", "submitted", "served", "rejected", "failed"];

  usePageHead(() => ({ sub: "Fertig gelöste Spots für die häufigsten Turnier-Situationen – gerechnet hier oder geladen aus Ringside Cloud." }));
</script>

<ClientLock feature="Die Bibliothek">
  <div class="page library">
    {#if error}<p class="err">{error}</p>{/if}

    {#if data && worker}
      <ol class="how" aria-label="So nutzt du die Bibliothek">
        <li><b>Situation wählen</b><span>zum Beispiel du am Button gegen den Big Blind</span></li>
        <li><b>Flop anklicken</b><span>die drei Karten nach der Preflop-Runde</span></li>
        <li><b>Nachsehen und üben</b><span>für jede Hand: checken, setzen oder aufgeben – was bringt am meisten?</span></li>
      </ol>

      <div class="top">
        <section class="panel overview" aria-label="Stand der Bibliothek">
          <div class="ring" style="--p:{totals.spots ? totals.done / totals.spots : 0}">
            <span class="num">{pct(totals.spots ? totals.done / totals.spots : 0)}</span>
          </div>
          <div class="otext">
            <p class="lead">
              <b class="num">{totals.done.toLocaleString("de-DE")}</b> von {totals.spots.toLocaleString("de-DE")} Spots sind fertig.
            </p>
            <p class="faint">
              In {totals.started} von {totals.situations} Situationen kannst du schon nachsehen.
              {#if totals.local > 0}{totals.local} davon hat dein PC selbst gerechnet.{/if}
            </p>
          </div>
          <div class="contrib">
            {#if data.share || totals.contributed > 0}
              <p class="lead">
                <b class="num">{totals.contributed}</b> von dir beigetragen, <b class="num">{totals.served}</b> für alle verfügbar
              </p>
              <div class="cbar" aria-hidden="true">
                <i class="ok" style="flex:{totals.served}"></i>
                <i class="wait" style="flex:{totals.contributed - totals.served}"></i>
                <i class="rest" style="flex:{totals.contributed ? 0 : 1}"></i>
              </div>
              <p class="faint">
                {#if totals.contributed === 0}
                  Was dein PC rechnet, lädt Ringside hoch, damit andere es nicht noch einmal rechnen müssen.
                {:else if totals.contributed > totals.served}
                  {totals.contributed - totals.served} sind noch nicht freigegeben; Ringside Cloud prüft ihren Aufbau einmal pro Stunde.
                {:else}
                  Alles, was du beigetragen hast, ist für alle verfügbar.
                {/if}
                {#if totals.waiting > 0}{totals.waiting} noch nicht hochgeladen, das holt Ringside nach.{/if}
                {#if totals.rejected > 0}{totals.rejected} abgelehnt (Grund im Tooltip der Kachel).{/if}
              </p>
            {:else}
              <p class="faint">Teilen ist aus: Was dein PC rechnet, bleibt bei dir. Ändern kannst du das in den Einstellungen unter Ringside Cloud.</p>
            {/if}
          </div>
        </section>

        <section class="panel worker" aria-label="Mitrechnen">
          <div class="panel-head"><h2 class="panel-title">Mitrechnen</h2></div>
          <div class="wbody">
            <RunControl {data} {worker} onChange={load} />
            <div class="auto">
              <span class="label">Automatik</span>
              <BackgroundMode mode={data.mode} onChange={setMode} />
            </div>
            <p class="why">
              Die Bibliothek wächst, weil Ringside-PCs Spots rechnen und teilen. Geteilt wird nur der Spot und seine Lösung,
              nichts über dich.
            </p>
            <details>
              <summary>Details für Experten</summary>
              <dl>
                {#if worker.state === "solving"}
                  <dt>Iteration</dt>
                  <dd class="num">{worker.iteration}</dd>
                  <dt>Ausbeutbarkeit</dt>
                  <dd class="num">{num(worker.exploitability_pct)} % des Pots, Ziel {num(data.target_pct)} %</dd>
                  <dt>Threads</dt>
                  <dd class="num">{worker.threads}</dd>
                {/if}
                <dt>Seit dem Start</dt>
                <dd>{worker.solved} gerechnet, {worker.fetched} geladen, {worker.shared} hochgeladen</dd>
                <dt>Auf der Festplatte</dt>
                <dd class="num">{mb(data.disk_bytes)}</dd>
                {#if worker.last_error}<dt>Zuletzt</dt><dd class="warn">{worker.last_error}</dd>{/if}
              </dl>
            </details>
          </div>
        </section>
      </div>

      <div class="body">
        <nav class="panel scenarios" aria-label="Situationen">
          {#each stacks as stack (stack)}
            <h3>{stack} BB <span>· {depthOf(stack)}</span></h3>
            {#each data.scenarios.filter((s) => s.stack_bb === stack) as s (s.id)}
              {@const cov = coverage(s)}
              <button class="scenario" class:on={s.id === selected} onclick={() => (selected = s.id)}>
                <span class="name">{titleOf(s)}</span>
                <span class="kind">{s.pot_type === "srp" ? "Normaler Raise" : "Nach einer 3-Bet"}</span>
                <span class="mini" aria-label="{pct(cov)} der Flops gelöst">
                  <i class="cloud" style="width:{(count(s, 'cloud') / s.status.length) * 100}%"></i>
                  <i class="local" style="width:{(count(s, 'local') / s.status.length) * 100}%"></i>
                </span>
                <span class="count num">{pct(cov)}</span>
              </button>
            {/each}
          {/each}
        </nav>

        {#if current}
          {@const cov = coverage(current)}
          <section class="panel flops" aria-label="Flops">
            <div class="fhead">
              <h2>{titleOf(current)}</h2>
              <p class="sub">{situationOf(current)}</p>
              <p>
                {storyOf(current)}
                Im Pot liegen {num(current.pot)} BB, jeder hat noch {num(current.stack)} BB. {firstOf(current)}
              </p>
              <p class="cover">
                {#if cov >= 0.995}
                  Für jeden Flop gibt es hier eine Lösung.
                {:else if cov > 0}
                  Schon für <b>{pct(cov)}</b> aller möglichen Flops gibt es hier eine Lösung. Klick auf einen farbigen Flop.
                {:else}
                  Hier ist noch nichts gelöst. Mitrechnen füllt die Bibliothek nach und nach.
                {/if}
              </p>
            </div>
            <div class="legend" aria-label="Legende">
              <div>
                <h4>Flop-Kachel</h4>
                <span><i class="dot cloud"></i>Heruntergeladen, fertig zum Ansehen</span>
                <span><i class="dot local"></i>Auf deinem PC gerechnet</span>
                <span><i class="dot open"></i>Noch offen</span>
              </div>
              <div>
                <h4>Symbol an deinen Spots</h4>
                {#each MARKS as m (m)}
                  <span><ShareMark mark={m} />{m === "only_here" && !data.share ? "Nur auf deinem PC, Teilen ist aus" : MARK_NAME[m]}</span>
                {/each}
              </div>
            </div>
            {#each groups as g (g.name)}
              <h3>{g.name} <span class="faint">– {TEXTURE_HINT[g.name]}</span></h3>
              <div class="grid">
                {#each g.flops as { flop, i } (flop)}
                  {@const st = current.status[i]}
                  {@const busy = worker.state === "solving" && worker.scenario === current.id && worker.flop === flop}
                  <button
                    class="flop {st ?? 'open'}"
                    class:busy
                    disabled={!st || opening !== null}
                    title={st === "local"
                      ? `${markText(current.shared[i], data.share)} Klick zum Ansehen.`
                      : st === "cloud"
                        ? "Heruntergeladen – klick zum Ansehen."
                        : busy
                          ? "Wird gerade gerechnet"
                          : "Noch nicht gelöst"}
                    onclick={() => open(current, flop)}
                  >
                    {#each cards(flop) as [r, s], k (k)}<span class="card {s}">{r}<small>{suit(s)}</small></span>{/each}
                    {#if st === "local"}<span class="badge"><ShareMark mark={markOf(current.shared[i])} /></span>{/if}
                  </button>
                {/each}
              </div>
            {/each}
            <details class="expert">
              <summary>Wie die Lösungen entstehen</summary>
              <p>
                Gelöst wird mit dem Solver von Ringside, bis kein Gegner mehr als {num(data.target_pct)} % des Pots gegen die
                Strategie gewinnen könnte (Ausbeutbarkeit). Die Ranges vor dem Flop stammen aus dem Preflop-Modell (9 Spieler,
                Big-Blind-Ante); Einsätze nach dem Flop: ein Drittel des Pots auf dem Flop, zwei Drittel bis drei Viertel auf Turn und River, dazu Raises und All-in.
                Gespeichert sind alle Flop-Entscheidungen beider Spieler je Hand und die erste Entscheidung auf jedem Turn je
                Handklasse; tiefer rechnet der Solver auf Wunsch neu.
              </p>
              <p>
                Die {data.flops.length} Flops sind eine geschichtete Stichprobe der 1755 strategisch verschiedenen Flops (Flops, die
                sich nur durch vertauschte Farben unterscheiden, spielen sich gleich). Jeder steht für etwa
                {Math.round(ALL_FLOPS / data.flops.length)} der {ALL_FLOPS.toLocaleString("de-DE")} möglichen Flops; daher die
                Prozentangaben. Heruntergeladene Spots hat die Community mit Ringside gerechnet; Ringside prüft vor der Nutzung, ob
                sie richtig aufgebaut sind, und rechnet Stichproben nach. Die Grundstock-Pakete tragen die Signatur von Ringside.
              </p>
            </details>
          </section>
        {/if}
      </div>
    {:else if !error}
      <p class="faint">Lädt …</p>
    {/if}
  </div>
</ClientLock>

<style>
  .err {
    color: #ff9d93;
  }
  .warn {
    color: var(--amber);
  }
  .how {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
    counter-reset: step;
  }
  .how li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 2px 10px;
    padding: 12px 14px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
    counter-increment: step;
  }
  .how li::before {
    content: counter(step);
    grid-row: span 2;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--surface-3);
    font-family: var(--font-num);
    font-weight: 600;
    font-size: 13px;
  }
  .how b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .how span {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .top {
    display: grid;
    grid-template-columns: minmax(280px, 400px) minmax(0, 1fr);
    gap: 14px;
    margin-bottom: 14px;
    align-items: start;
  }
  .overview {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 16px 18px;
    align-items: center;
    padding: 18px;
  }
  .contrib {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .contrib p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.45;
  }
  .contrib .lead {
    font-size: 14px;
  }
  .cbar {
    display: flex;
    height: 6px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--surface-3);
  }
  .cbar i {
    display: block;
  }
  .cbar .ok {
    background: var(--pos);
  }
  .cbar .wait {
    background: var(--amber);
  }
  .ring {
    --p: 0;
    flex: none;
    display: grid;
    place-items: center;
    width: 84px;
    height: 84px;
    border-radius: 50%;
    background:
      radial-gradient(closest-side, var(--surface) 76%, transparent 77%),
      conic-gradient(var(--sd) calc(var(--p) * 100%), var(--surface-3) 0);
  }
  .ring span {
    font-size: 19px;
    font-weight: 600;
  }
  .otext {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .otext p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.45;
  }
  .otext .lead {
    font-size: 14px;
    color: var(--text);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 36px;
    margin-top: 14px;
    padding: 12px 14px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: 12.5px;
    color: var(--text-2);
  }
  .legend div {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 16px;
  }
  .legend h4 {
    flex-basis: 100%;
    margin: 0;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-3);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  i.cloud,
  .dot.cloud {
    background: var(--sd);
  }
  i.local,
  .dot.local {
    background: var(--pos);
  }
  .dot.open {
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  .wbody {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 14px 18px 16px;
  }
  .auto {
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .auto .label {
    padding-top: 6px;
    color: var(--text-2);
    font-size: 12.5px;
    font-weight: 600;
  }
  .why {
    margin: 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.5;
    max-width: 75ch;
  }
  details summary {
    cursor: pointer;
    color: var(--text-3);
    font-size: 12.5px;
  }
  details summary:hover {
    color: var(--text-2);
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 4px 14px;
    margin: 8px 0 0;
    font-size: 12.5px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }

  .body {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .scenarios {
    display: flex;
    flex-direction: column;
    padding: 8px;
    position: sticky;
    top: 12px;
  }
  .scenarios h3 {
    margin: 12px 10px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .scenarios h3 span {
    color: var(--text-3);
    font-weight: 400;
  }
  .scenario {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 3px 10px;
    padding: 9px 10px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .scenario:hover {
    background: var(--surface-2);
  }
  .scenario.on {
    background: var(--surface-3);
  }
  .name {
    grid-column: 1 / -1;
    font-size: 13.5px;
    font-weight: 500;
  }
  .kind {
    grid-column: 1 / -1;
    color: var(--text-3);
    font-size: 12px;
  }
  .mini {
    display: flex;
    height: 4px;
    overflow: hidden;
    border-radius: 99px;
    background: var(--surface-3);
  }
  .mini i {
    display: block;
    height: 100%;
  }
  .count {
    color: var(--text-2);
    font-size: 12px;
    min-width: 3.2em;
    text-align: right;
  }

  .flops {
    padding: 18px;
  }
  .fhead h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
  .fhead p {
    margin: 4px 0 0;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.5;
  }
  .fhead .sub {
    margin-top: 2px;
    color: var(--text-3);
  }
  .fhead .cover {
    margin-top: 10px;
    color: var(--text);
  }
  .flops h3 {
    margin: 18px 0 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .flops h3 span {
    font-weight: 400;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
    gap: 6px;
  }
  .flop {
    position: relative;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 3px;
    height: 34px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--bg);
    cursor: pointer;
    font-family: var(--font-num);
    font-size: 14px;
    font-weight: 600;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .flop.local {
    border-color: color-mix(in srgb, var(--pos) 45%, transparent);
    background: color-mix(in srgb, var(--pos) 10%, var(--surface));
  }
  .flop.cloud {
    border-color: color-mix(in srgb, var(--sd) 45%, transparent);
    background: color-mix(in srgb, var(--sd) 10%, var(--surface));
  }
  .flop:not(:disabled):hover {
    background: var(--surface-3);
    border-color: var(--text-2);
  }
  .flop.open {
    opacity: 0.4;
    cursor: default;
  }
  .flop.busy {
    opacity: 1;
    border-style: dashed;
    border-color: var(--pos);
  }
  .badge {
    position: absolute;
    top: -6px;
    right: -6px;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--bg);
  }
  .card small {
    font-size: 11px;
    margin-left: 1px;
  }
  .card.s small {
    color: var(--text);
  }
  .card.h small {
    color: var(--neg);
  }
  .card.d small {
    color: var(--sd);
  }
  .card.c small {
    color: var(--pos);
  }
  .expert {
    margin-top: 20px;
  }
  .expert p {
    margin: 8px 0 0;
    color: var(--text-2);
    font-size: 12.5px;
    line-height: 1.55;
    max-width: 90ch;
  }
  /* Very wide windows: the flop grid gets the room; reading text keeps its own measure. */
  @container (min-width: 1800px) {
    .library {
      max-width: 1760px;
    }
    /* Power and automatic sit beside the button instead of under it. */
    .wbody {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      gap: 16px 32px;
    }
    .wbody > :global(.run) {
      grid-row: span 2;
      align-self: start;
    }
    .wbody > .why,
    .wbody > details {
      grid-column: 1 / -1;
    }
  }
  @media (max-width: 980px) {
    .how,
    .top,
    .body {
      grid-template-columns: 1fr;
    }
    .scenarios {
      position: static;
    }
  }
</style>
