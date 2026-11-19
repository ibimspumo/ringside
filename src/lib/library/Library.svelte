<script lang="ts">
  import { onMount } from "svelte";
  import { inTauri } from "../api";
  import ClientLock from "../app/ClientLock.svelte";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";
  import Dialog from "../ui/Dialog.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import MiniCard from "../ui/MiniCard.svelte";
  import { usePageHead } from "../ui/page.svelte";
  import ProgressBar from "../ui/ProgressBar.svelte";
  import ProgressRing from "../ui/ProgressRing.svelte";
  import SectionHead from "../ui/SectionHead.svelte";
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import Term from "../ui/Term.svelte";
  import Tooltip from "../ui/Tooltip.svelte";
  import { libraryApi, onLibraryStatus } from "./api";
  import BackgroundMode from "./BackgroundMode.svelte";
  import { preview } from "./demo";
  import { cards, depthOf, firstOf, flopName, markOf, markText, type Mark, situationOf, storyOf, titleOf } from "./labels";
  import { openInSolver } from "./open";
  import PowerChoice from "./PowerChoice.svelte";
  import RunControl from "./RunControl.svelte";
  import ShareMark from "./ShareMark.svelte";
  import type { LibraryMode, LibraryOverview, LibraryScenario, WorkerStatus } from "./types";

  /** Switches to the solver page once a spot is loaded there. */
  let { onOpen }: { onOpen: () => void } = $props();

  let data = $state<LibraryOverview | null>(null);
  let worker = $state<WorkerStatus | null>(null);
  let selected = $state<string | null>(null);
  let error = $state<string | null>(null);
  let openError = $state<string | null>(null);
  let opening = $state<string | null>(null);
  let tuning = $state(!inTauri && preview.has("settings"));

  async function load() {
    try {
      const o = await libraryApi.overview();
      data = o;
      worker = o.worker;
      error = null;
      if (!selected || !o.scenarios.some((s) => s.id === selected)) selected = o.scenarios[0]?.id ?? null;
    } catch (e) {
      error = message(e);
    }
  }
  const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

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
      openError = null;
      onOpen();
    } catch (e) {
      openError = message(e);
    } finally {
      opening = null;
    }
  }

  /** Any solved flop of the situation, for a first look without choosing. */
  function openAny(s: LibraryScenario) {
    const solved = data?.flops.filter((_, i) => s.status[i]) ?? [];
    if (solved.length) open(s, solved[Math.floor(Math.random() * solved.length)]);
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
    const pending = n("submitted");
    return {
      local,
      spots,
      done: local + cloud,
      started,
      situations: all.length,
      contributed: served + pending,
      served,
      pending,
      rejected: n("rejected"),
      onlyHere: n("only_here"),
      failed: n("failed"),
    };
  });

  const current = $derived(data?.scenarios.find((s) => s.id === selected) ?? null);
  const stacks = $derived([...new Set((data?.scenarios ?? []).map((s) => s.stack_bb))].sort((a, b) => a - b));
  const stackOptions = $derived(stacks.map((s) => ({ value: s, label: `${s} BB`, hint: depthOf(s) })));
  const POTS: { id: LibraryScenario["pot_type"]; name: string; hint: string }[] = [
    { id: "srp", name: "Normaler Raise", hint: "Du erhöhst, einer geht mit" },
    { id: "three_bet", name: "Nach einer 3-Bet", hint: "Du erhöhst, einer erhöht noch einmal, du gehst mit" },
  ];
  /** The situation with the most solutions at a depth, so a new depth opens on something to see. */
  function pickStack(stack: number) {
    const at = (data?.scenarios ?? []).filter((s) => s.stack_bb === stack);
    selected = [...at].sort((a, b) => coverage(b) - coverage(a))[0]?.id ?? selected;
  }
  /** Fullest situation of all, as the way out of an empty one. */
  const fullest = $derived([...(data?.scenarios ?? [])].sort((a, b) => coverage(b) - coverage(a))[0] ?? null);

  type Texture = "Rainbow" | "Zweifarbig" | "Monoton" | "Gepaart" | "Drilling";
  const TEXTURE_HINT: Record<Texture, string> = {
    Rainbow: "drei Farben, noch kein Flush‑Draw",
    Zweifarbig: "zwei Karten einer Farbe, Flush‑Draws möglich",
    Monoton: "alle drei in einer Farbe, ein Flush ist schon möglich",
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
  /** Marks that occur in the situation on view; the legend names only those. */
  const marksHere = $derived.by(() => {
    if (!current) return [];
    const seen = new Set(current.status.flatMap((st, i) => (st === "local" ? [markOf(current.shared[i])] : [])));
    return (["served", "submitted", "only_here", "rejected", "failed"] as Mark[]).filter((m) => seen.has(m));
  });
  const MARK_SHORT: Record<Mark, string> = {
    served: "verteilt",
    submitted: "noch nicht freigegeben",
    only_here: "noch nicht hochgeladen",
    rejected: "abgelehnt oder zurückgezogen",
    failed: "neuer Versuch folgt",
  };

  const pct = (x: number) => `${Math.round(x * 100)} %`;
  const int = (x: number) => x.toLocaleString("de-DE");
  const num = (x: number) => x.toLocaleString("de-DE", { maximumFractionDigits: 2 });
  const mb = (b: number) =>
    b >= 1e9 ? `${(b / 1e9).toLocaleString("de-DE", { maximumFractionDigits: 1 })} GB` : `${Math.round(b / 1e6).toLocaleString("de-DE")} MB`;

  function tip(s: LibraryScenario, i: number, busy: boolean): string {
    const st = s.status[i];
    if (busy) return "Wird gerade gerechnet.";
    if (st === "local") return `${markText(s.shared[i], data?.share ?? false)} Klick zum Ansehen.`;
    if (st === "cloud") return "Fertig, aus der Cloud geladen, von der Community gerechnet. Klick zum Ansehen.";
    return "Noch nicht gelöst. Mitrechnen füllt die Bibliothek nach und nach.";
  }

  usePageHead(() => ({ sub: "Fertig gelöste Spots für die häufigsten Turnier-Situationen – gerechnet hier oder geladen aus Ringside Cloud." }));
</script>

<ClientLock feature="Die Bibliothek">
  <div class="page library">
    {#if data && worker}
      <Card pad="md">
        <div class="band">
          <section class="stand" aria-label="Stand der Bibliothek">
            <h2 class="label">Stand der Bibliothek</h2>
            <div class="ringrow">
              <ProgressRing value={totals.spots ? totals.done / totals.spots : 0} size={56} stroke={5} tone="info" label="Anteil fertiger Spots">
                <span class="ring-num num">{pct(totals.spots ? totals.done / totals.spots : 0)}</span>
              </ProgressRing>
              <div class="facts">
                <p class="lead">
                  <b class="num">{int(totals.done)}</b> von <span class="num">{int(totals.spots)}</span>
                  <Term tip="Ein Spot ist eine Situation mit einem bestimmten Flop, zum Beispiel du am Button gegen den Big Blind auf K♠ 7♦ 2♣.">Spots</Term> fertig
                </p>
                <p class="meta">In <b class="num">{totals.started}</b> von {totals.situations} Situationen kannst du schon nachsehen.</p>
              </div>
            </div>
          </section>

          <section class="worker" aria-label="Mitrechnen">
            <h2 class="label">Mitrechnen</h2>
            <RunControl {data} {worker} primary={totals.done === 0} onChange={load}>
              {#snippet after()}
                <Button variant="quiet" aria-haspopup="dialog" onclick={() => (tuning = true)}>
                  {#snippet icon()}
                    <svg class="gear" viewBox="0 0 16 16" width="14" height="14"><path d="M2.5 4.5h6M11.5 4.5h2M2.5 11.5h2M7.5 11.5h6" /><circle cx="10" cy="4.5" r="1.5" /><circle cx="6" cy="11.5" r="1.5" /></svg>
                  {/snippet}
                  Kraft und Automatik
                </Button>
              {/snippet}
            </RunControl>
          </section>
          <div class="yours" aria-label="Dein Beitrag">
              {#if !data.share}
                <p class="meta">Teilen ist aus: Was dein PC rechnet, bleibt bei dir. Einschalten kannst du es in den Einstellungen unter Ringside Cloud.</p>
              {:else if totals.contributed === 0 && totals.onlyHere + totals.failed === 0}
                <p class="meta">Was dein PC rechnet, lädt Ringside hoch, damit andere es nicht noch einmal rechnen müssen.</p>
              {:else}
                <ul class="keys">
                  <li class="who">Von dir: <b class="num">{int(totals.contributed)}</b> geteilt</li>
                  <li>
                    <ShareMark mark="served" /><span><b class="num">{int(totals.served)}</b> <Term tip="Ringside Cloud hat deine Lösung angenommen und verteilt sie an alle. Angenommen wird, was richtig aufgebaut ist; nachgerechnet werden nur Stichproben.">{MARK_SHORT.served}</Term> <i>– für alle da</i></span>
                  </li>
                  {#if totals.pending > 0}
                    <li>
                      <ShareMark mark="submitted" /><span><b class="num">{int(totals.pending)}</b> <Term tip="Hochgeladen; Ringside Cloud schaut noch, ob die Lösung richtig aufgebaut ist. Danach bekommen sie alle.">{MARK_SHORT.submitted}</Term></span>
                    </li>
                  {/if}
                  {#if totals.onlyHere > 0}
                    <li>
                      <Tooltip text="Auf deinem PC gerechnet, aber noch nicht hochgeladen. Das holt Ringside von selbst nach.">
                        <span class="item"><ShareMark mark="only_here" /><span><b class="num">{totals.onlyHere}</b> {MARK_SHORT.only_here}</span></span>
                      </Tooltip>
                    </li>
                  {/if}
                  {#if totals.failed > 0}
                  <li>
                    <Tooltip text="Das Hochladen klappte nicht, etwa weil keine Verbindung bestand. Ringside versucht es von selbst noch einmal.">
                      <span class="item"><ShareMark mark="failed" /><span><b class="num">{totals.failed}</b> {MARK_SHORT.failed}</span></span>
                    </Tooltip>
                  </li>
                {/if}
                {#if totals.rejected > 0}
                    <li>
                      <Tooltip text="Ringside Cloud hat die Lösung nicht angenommen oder später zurückgezogen, etwa weil eine Stichprobe von diesem Internetanschluss beim Nachrechnen falsch war. Den Grund zeigt die Kachel mit dem roten Kreuz.">
                        <span class="item"><ShareMark mark="rejected" /><span><b class="num">{totals.rejected}</b> {MARK_SHORT.rejected}</span></span>
                      </Tooltip>
                    </li>
                  {/if}
                </ul>
              {/if}
            </div>
        </div>
      </Card>

      <Dialog bind:open={tuning} size="lg" title="Kraft und Automatik" sub="Wie viel dein PC für die Bibliothek rechnet – und wann.">
        <div class="tuning">
          <div class="row">
            <h3 class="label">Kraft</h3>
            <PowerChoice {data} />
          </div>
          <div class="row">
            <h3 class="label">Automatik</h3>
            <BackgroundMode mode={data.mode} onChange={setMode} />
          </div>
          <div class="row">
            <span></span>
            <div class="why">
              <p>Die Bibliothek wächst, weil Ringside-PCs Spots rechnen und teilen. Geteilt wird nur der Spot und seine Lösung, nichts über dich.</p>
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
          </div>
        </div>
        {#snippet actions()}<Button variant="primary" onclick={() => (tuning = false)}>Fertig</Button>{/snippet}
      </Dialog>

      <div class="body">
        <nav class="situations" aria-label="Situationen">
          <div class="steprow"><p class="step"><span class="n num">1</span>Situation wählen</p></div>
          {#if stacks.length > 1 && current}
            <div class="depth">
              <SegmentedControl label="Stacktiefe" value={current.stack_bb} options={stackOptions} onchange={pickStack} />
              <p>{depthOf(current.stack_bb).replace(/^./, (c) => c.toUpperCase())} – so viele Big Blinds hat jeder.</p>
            </div>
          {/if}
          {#each POTS as pot (pot.id)}
            {@const list = data.scenarios.filter((s) => s.stack_bb === current?.stack_bb && s.pot_type === pot.id)}
            {#if list.length}
              <div class="pot">
                <h3>{pot.name} <span>· {pot.hint}</span></h3>
                {#each list as s (s.id)}
                  {@const cov = coverage(s)}
                  <Card pad="sm" selected={s.id === selected} onclick={() => (selected = s.id)}>
                    <span class="sit">
                      <span class="sname">{titleOf(s)}</span>
                      <span class="sbar">
                        <ProgressBar value={cov} tone="info" label="Anteil gelöster Flops" />
                        <span class="num" class:none={cov === 0}>{cov === 0 ? "leer" : pct(cov)}</span>
                      </span>
                    </span>
                  </Card>
                {/each}
              </div>
            {/if}
          {/each}
        </nav>

        {#if current}
          {@const cov = coverage(current)}
          <div class="flopcol">
          <div class="steprow">
            <p class="step"><span class="n num">2</span>Flop anklicken und nachsehen</p>
            {#if cov > 0}
              <Button variant="primary" loading={opening !== null} onclick={() => openAny(current)}>Zufälligen Flop ansehen</Button>
            {/if}
          </div>
          <Card pad="lg">
            <section class="flops" aria-label="Flops">
              <SectionHead title={titleOf(current)} sub="{situationOf(current)} · Pot {num(current.pot)} BB · jeder hat noch {num(current.stack)} BB" />
              <p class="story">
                {storyOf(current)} {firstOf(current)}
                {#if cov > 0}
                  <span class="cover">
                    {cov >= 0.995 ? "Für jeden Flop gibt es hier eine Lösung." : `Für ${pct(cov)} aller möglichen Flops gibt es hier schon eine Lösung.`}
                    <span class="howto">Ein Klick auf einen hellen Flop zeigt dir für jede Hand, ob Checken, Setzen oder Aufgeben am meisten bringt.</span>
                  </span>
                {/if}
              </p>
              {#if cov === 0}
                <EmptyState
                  inline
                  compact
                  title="Hier ist noch nichts gelöst"
                  text={totals.done === 0
                    ? "Die Bibliothek ist noch leer. Ringside lädt fertige Spots aus der Cloud, und Mitrechnen füllt sie nach und nach."
                    : "Mitrechnen füllt die Bibliothek nach und nach. In anderen Situationen gibt es schon fertige Flops."}
                >
                  {#snippet action()}
                    {#if fullest && coverage(fullest) > 0 && fullest.id !== current.id}
                      <Button onclick={() => (selected = fullest.id)}>Zur vollsten Situation</Button>
                    {/if}
                  {/snippet}
                </EmptyState>
              {/if}
              {#if openError}<p class="err" role="alert">Der Spot ließ sich nicht öffnen: {openError}</p>{/if}

              <div class="legend" aria-label="Legende">
                <span><MiniCard card="As" />Fertig</span>
                <span><MiniCard card="As" dim />Noch offen</span>
                {#if marksHere.length}
                  <span class="mine">
                    <span class="from">Deine Spots:</span>
                    {#each marksHere as m (m)}<span><ShareMark mark={m} />{MARK_SHORT[m]}</span>{/each}
                  </span>
                {/if}
              </div>

              {#each groups as g (g.name)}
                {@const solved = g.flops.filter(({ i }) => current.status[i]).length}
                <h3 class="texture">
                  <span>{g.name} <span class="hint">– {TEXTURE_HINT[g.name]}</span></span>
                  <span class="gcount num">{solved} von {g.flops.length} fertig</span>
                </h3>
                <div class="grid">
                  {#each g.flops as { flop, i } (flop)}
                    {@const st = current.status[i]}
                    {@const busy = worker.state === "solving" && worker.scenario === current.id && worker.flop === flop}
                    <div class="cell">
                      <Tooltip text={tip(current, i, busy)}>
                        <Card
                          pad="xs"
                          disabled={!st || opening !== null}
                          aria-label="Flop {flopName(flop)}{st ? '' : ', noch nicht gelöst'}"
                          onclick={() => open(current, flop)}
                        >
                          <span class="tile" class:open={!st}>
                            {#each cards(flop) as [r, s], k (k)}<MiniCard card={r + s} />{/each}
                            {#if busy || opening === flop}
                              <span class="mark"><ProgressRing value={null} size={14} stroke={2} label="Wird gerechnet" /></span>
                            {:else if st === "local"}
                              <span class="mark"><ShareMark mark={markOf(current.shared[i])} /></span>
                            {/if}
                          </span>
                        </Card>
                      </Tooltip>
                    </div>
                  {/each}
                </div>
              {/each}

              <details class="expert">
                <summary>Wie die Lösungen entstehen</summary>
                <p>
                  Gelöst wird mit dem Solver von Ringside, bis kein Gegner mehr als {num(data.target_pct)} % des Pots gegen die Strategie
                  gewinnen könnte (Ausbeutbarkeit). Die Ranges vor dem Flop stammen aus dem Preflop-Modell (9 Spieler, Big-Blind-Ante);
                  Einsätze nach dem Flop: ein Drittel des Pots auf dem Flop, zwei Drittel bis drei Viertel auf Turn und River, dazu Raises und
                  All-in. Gespeichert sind alle Flop-Entscheidungen beider Spieler je Hand und die erste Entscheidung auf jedem Turn je
                  Handklasse; tiefer rechnet der Solver auf Wunsch neu.
                </p>
                <p>
                  Die {data.flops.length} Flops sind eine geschichtete Stichprobe der 1755 strategisch verschiedenen Flops (Flops, die sich nur
                  durch vertauschte Farben unterscheiden, spielen sich gleich). Jeder steht für etwa {Math.round(ALL_FLOPS / data.flops.length)}
                  der {int(ALL_FLOPS)} möglichen Flops; daher die Prozentangaben. Heruntergeladene Spots hat die Community mit Ringside
                  gerechnet; Ringside prüft vor der Nutzung, ob sie richtig aufgebaut sind, und rechnet Stichproben nach. Die
                  Grundstock-Pakete tragen die Signatur von Ringside.
                </p>
              </details>
            </section>
          </Card>
          </div>
        {/if}
      </div>
    {:else if error}
      <div class="state">
      <EmptyState
        tone="error"
        title="Die Bibliothek ließ sich nicht laden"
        text="{error} Meist sperrt ein Virenscanner oder ein fehlendes Schreibrecht den Datenordner von Ringside: %APPDATA%\app.ringside.hud (in die Adresszeile des Explorers einfügen). Gib ihn im Virenscanner frei und versuch es noch einmal; hilft das nicht, starte Ringside neu."
      >
        {#snippet action()}<Button variant="primary" onclick={load}>Noch einmal versuchen</Button>{/snippet}
      </EmptyState>
      </div>
    {:else}
      <div class="state"><EmptyState tone="loading" title="Bibliothek wird geladen" text="Das dauert nur einen Moment." /></div>
    {/if}
  </div>
</ClientLock>

<style>
  .library {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  p {
    margin: 0;
  }
  .warn {
    color: var(--amber);
  }
  .err {
    color: var(--neg);
    font-size: var(--fs-md);
  }

  /* Like the lock of the page: error and loading sit in the middle of the window. */
  .state {
    display: grid;
    place-items: center;
    min-height: calc(100vh - 220px);
  }

  /* The band: where the library stands, what you gave, and the worker. */
  .band {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
    gap: var(--sp-4) var(--sp-6);
  }
  .band > section + section {
    padding-left: var(--sp-6);
    border-left: 1px solid var(--line);
  }
  .ringrow {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
  }
  .ring-num {
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .facts,
  .stand,
  .worker {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }
  .label {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .lead {
    color: var(--text);
    font-size: var(--fs-base);
    line-height: var(--lh-tight);
  }
  .lead b {
    font-weight: var(--fw-semibold);
  }
  .meta {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .meta b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .yours {
    grid-column: 1 / -1;
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-1) var(--sp-4);
    margin: 0;
    padding: 0;
    list-style: none;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .keys li,
  .keys .item {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }
  .keys b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .keys i {
    font-style: normal;
  }
  .gear {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
  }
  .tuning {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
  }
  .row {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .row > .label {
    padding-top: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .why {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    max-width: 75ch;
  }
  details summary {
    cursor: pointer;
    color: var(--text-3);
    font-size: var(--fs-md);
    transition: color var(--dur-fast) var(--ease-standard);
  }
  details summary:hover {
    color: var(--text-2);
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--sp-1) var(--sp-4);
    margin: var(--sp-2) 0 0;
    font-size: var(--fs-md);
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }

  /* Situation on the left, its flops on the right. */
  .body {
    display: grid;
    grid-template-columns: 272px minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  .step {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .flopcol {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .steprow {
    min-height: 32px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
  }
  .step .n {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
    color: var(--text);
    font-size: var(--fs-sm);
  }
  .situations {
    position: sticky;
    top: var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .depth {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .depth p {
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-base);
  }
  .pot {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .pot h3 {
    margin: var(--sp-2) 0 0;
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
  }
  .pot h3 span {
    color: var(--text-2);
    font-weight: var(--fw-regular);
  }
  .sit {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .sname {
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
    line-height: var(--lh-tight);
  }
  .sbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 3.2em;
    align-items: center;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: right;
  }
  .sbar .none {
    color: var(--text-3);
  }

  .flops {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .story {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    max-width: 90ch;
  }
  .cover {
    color: var(--text);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--r-sm);
    background: var(--surface-sunken);
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }
  /* Your own marks wrap as one group, so the line never breaks inside it. */
  .legend .mine {
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
    padding-left: var(--sp-4);
    border-left: 1px solid var(--line-2);
  }
  /* Availability first, then a clear break before the marks of your own spots. */
  .legend .from {
    color: var(--text-2);
  }
  .texture {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-1) var(--sp-4);
    margin: var(--sp-3) 0 0;
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  .texture .hint {
    color: var(--text-2);
    font-weight: var(--fw-regular);
  }
  .gcount {
    color: var(--text-2);
    font-size: var(--fs-sm);
    font-weight: var(--fw-regular);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    gap: var(--sp-2);
  }
  /* A grid cell blockifies the tooltip's anchor, so every tile fills its column. */
  .cell {
    display: grid;
  }
  .tile {
    position: relative;
    display: flex;
    justify-content: center;
    gap: 3px;
    font-size: var(--fs-base);
  }
  .mark {
    position: absolute;
    top: -12px;
    right: -12px;
    display: grid;
    place-items: center;
    line-height: 0;
  }
  .expert p {
    margin-top: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    max-width: 90ch;
  }

  @container (max-width: 1100px) {
    .band {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
      column-gap: var(--sp-4);
    }
    .band > section + section {
      padding-left: var(--sp-4);
    }
    .body {
      grid-template-columns: 236px minmax(0, 1fr);
    }
    .flops {
      margin: calc(-1 * var(--sp-2));
    }
    .keys i,
    .howto {
      display: none;
    }
  }
</style>
