<script lang="ts">
  import { MARK_NAME, markOf, markText } from "../library/labels";
  import ShareMark from "../library/ShareMark.svelte";
  import PlayingCard from "../PlayingCard.svelte";
  import { openSaved, solutionsApi, type SavedSolution, type SolutionsView } from "./solutions";
  import { solver } from "./store.svelte";
  import Term from "./Term.svelte";
  import Button from "../ui/Button.svelte";
  import { cap, precision } from "./explain";

  let data = $state<SolutionsView | null>(null);
  let error = $state<string | null>(null);
  let opening = $state<number | null>(null);
  let confirm = $state<number | null>(null);
  let pruned = $state<string | null>(null);

  const LIMITS = [100, 250, 500, 1000, 2000, 5000];
  const SOURCE: Record<SavedSolution["source"], string> = {
    own: "selbst aufgebaut",
    hand: "aus einer Hand",
    library: "aus der Bibliothek",
    training: "fürs Training",
  };

  function load() {
    solutionsApi
      .list()
      .then((d) => {
        data = d;
        error = null;
      })
      .catch((e) => (error = String(e)));
  }
  // Also after a solve finished and was saved.
  $effect(() => {
    void solver.savedId;
    load();
  });

  async function open(id: number) {
    opening = id;
    try {
      await openSaved(id);
    } catch (e) {
      error = String(e);
    } finally {
      opening = null;
    }
  }

  async function pin(s: SavedSolution) {
    await solutionsApi.pin(s.id, !s.pinned);
    load();
  }

  async function remove(id: number) {
    if (confirm !== id) {
      confirm = id;
      return;
    }
    confirm = null;
    await solutionsApi.delete(id);
    load();
  }

  async function setLimit(mb: number) {
    const n = await solutionsApi.setLimit(mb);
    pruned = n ? `${n} ältere ${n === 1 ? "Lösung" : "Lösungen"} gelöscht, damit alles in ${mbText(mb)} passt.` : null;
    load();
  }

  const cards = (b: string) => b.match(/.{2}/g) ?? [];
  const street = (b: string) => (b.length >= 10 ? "River" : b.length >= 8 ? "Turn" : "Flop");
  const num = (x: number, d = 1) => x.toLocaleString("de-DE", { maximumFractionDigits: d });
  const mbText = (mb: number) => (mb >= 1000 ? `${num(mb / 1000)} GB` : `${mb} MB`);
  function size(bytes: number) {
    if (bytes < 1e6) return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
    if (bytes < 1e9) return `${num(bytes / 1e6)} MB`;
    return `${num(bytes / 1e9)} GB`;
  }
  function when(secs: number) {
    const d = new Date(secs * 1000);
    const today = new Date();
    const time = d.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });
    if (d.toDateString() === today.toDateString()) return `heute, ${time}`;
    const y = new Date(today.getTime() - 86400000);
    if (d.toDateString() === y.toDateString()) return `gestern, ${time}`;
    return d.toLocaleDateString("de-DE", { day: "numeric", month: "short", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
  }

  const used = $derived(data ? data.bytes / (data.max_mb * 2 ** 20) : 0);
</script>

<section class="saved">
  <header class="bar">
    <div class="intro">
      <p>
        Jede Lösung, die du rechnest, landet hier und öffnet sich später sofort, ohne neu zu rechnen. Löst du denselben Spot
        noch einmal, nimmt Ringside die gespeicherte Lösung.
      </p>
    </div>
    {#if data}
      <div class="disk">
        <div class="disk-row">
          <span class="num"><b>{size(data.bytes)}</b> von {mbText(data.max_mb)}</span>
          <label>
            Platz
            <select value={data.max_mb} onchange={(e) => setLimit(Number((e.currentTarget as HTMLSelectElement).value))}>
              {#each LIMITS.includes(data.max_mb) ? LIMITS : [...LIMITS, data.max_mb].sort((a, b) => a - b) as mb (mb)}
                <option value={mb}>{mbText(mb)}</option>
              {/each}
            </select>
          </label>
        </div>
        <div class="meter" role="meter" aria-label="Belegter Platz" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(used * 100)}>
          <i style="width:{Math.min(100, used * 100)}%"></i>
        </div>
        <small>Wird es voll, gehen die ältesten zuerst. Angeheftete bleiben.</small>
      </div>
    {/if}
  </header>

  {#if pruned}<p class="note">{pruned}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}

  {#if data && !data.items.length}
    <div class="empty">
      <div class="ghost-cards" aria-hidden="true">
        <PlayingCard hidden size={40} /><PlayingCard hidden size={40} /><PlayingCard hidden size={40} />
      </div>
      <p><b>Noch keine Lösung gespeichert.</b> Bau einen Spot auf und lös ihn – er erscheint danach hier.</p>
      <Button variant="primary" onclick={() => (solver.view = "setup")}>Spot aufbauen</Button>
    </div>
  {:else if data}
    <ul class="grid">
      {#each data.items as s (s.id)}
        <li class="card" class:on={solver.savedId === s.id} class:pinned={s.pinned}>
          <div class="board" aria-label="Board {s.board}">
            {#each cards(s.board) as c (c)}<PlayingCard card={c} size={34} />{/each}
            <span class="street">ab {street(s.board)}</span>
          </div>
          <div class="who">
            <h3>{s.title}</h3>
            <p class="seats">
              <span><Term term="oop">{s.oop || "OOP"}</Term> handelt zuerst</span>
              <span><Term term="ip">{s.ip || "IP"}</Term> handelt zuletzt</span>
            </p>
          </div>
          <dl class="facts">
            <div><dt>Stack</dt><dd class="num">{num(s.stack_bb)} BB</dd></div>
            <div><dt>Pot</dt><dd class="num">{num(s.pot_bb)} BB</dd></div>
            <div>
              <dt><Term term="exploit">Genauigkeit</Term></dt>
              <dd class="num" title="Abweichung vom perfekten Spiel: {num(s.exploitability_pct, 2)} % des Pots">{cap(precision(s.exploitability_pct))}</dd>
            </div>
          </dl>
          <p class="line"><span>Größen</span>{s.sizes}</p>
          <p class="line"><span>Ranges</span>{s.ranges || "eigene"}</p>
          {#if s.shared}
            <p class="cloud" title={markText(s.shared, data.share)}>
              <ShareMark mark={markOf(s.shared)} />{MARK_NAME[markOf(s.shared)]}
            </p>
          {/if}
          <p class="meta">
            {when(s.created_at)} · {SOURCE[s.source]} · {size(s.bytes)}
            {#if !s.complete}· <span title="Tiefer rechnet Ringside bei Bedarf neu.">{street(s.board)} vollständig, danach nur der Einstieg</span>{/if}
          </p>
          <div class="acts">
            <span class="grow">
              <Button variant="primary" full disabled={opening !== null} loading={opening === s.id} onclick={() => open(s.id)}>
                {solver.savedId === s.id ? "Offen – ansehen" : "Öffnen"}
              </Button>
            </span>
            <Button
              variant="quiet"
              class={s.pinned ? "pinned" : ""}
              aria-pressed={s.pinned}
              title={s.pinned ? "Nicht mehr anheften" : "Anheften: wird beim Aufräumen nie gelöscht"}
              onclick={() => pin(s)}
            >
              {#snippet icon()}<svg class="pin" class:on={s.pinned} viewBox="0 0 16 16" width="15" height="15"><path d="M6 2.5h4l-.5 4 2.5 2.5H4l2.5-2.5zM8 9v4.5" /></svg>{/snippet}
              {s.pinned ? "Angeheftet" : "Anheften"}
            </Button>
            <Button variant={confirm === s.id ? "danger" : "danger-quiet"} onclick={() => remove(s.id)} onblur={() => confirm === s.id && (confirm = null)}>
              {confirm === s.id ? "Wirklich löschen?" : "Löschen"}
            </Button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .saved {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .bar {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px 32px;
    flex-wrap: wrap;
  }
  .intro {
    max-width: 62ch;
  }

  .intro p {
    margin: 4px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .disk {
    display: grid;
    gap: 6px;
    width: 300px;
  }
  .disk-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .disk-row b {
    color: var(--text);
    font-weight: 600;
  }
  label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
  }
  select {
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--line-2);
    border-radius: var(--r-sm);
    background: var(--surface-2);
  }
  .meter {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .meter i {
    display: block;
    height: 100%;
    background: var(--text-2);
  }
  .disk small {
    font-size: 12px;
    color: var(--text-2);
  }
  .note,
  .err {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .err {
    color: #ff9d93;
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: 12px;
    padding: 48px 24px;
    border: 1px dashed var(--line-2);
    border-radius: var(--r);
    color: var(--text-2);
    text-align: center;
  }
  .empty b {
    color: var(--text);
  }
  .empty p {
    margin: 0;
  }
  .ghost-cards {
    display: flex;
    gap: 4px;
    opacity: 0.6;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 14px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--surface);
  }
  .card.on {
    border-color: var(--line-2);
    background: var(--surface-2);
  }
  .board {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .street {
    margin-left: auto;
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-3);
  }
  .card.pinned .street::before {
    content: "angeheftet · ";
    color: var(--text-2);
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .seats {
    display: flex;
    gap: 14px;
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-3);
  }
  .seats :global(.term) {
    color: var(--text);
    font-weight: 600;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin: 0;
    padding: 8px 0;
    border-block: 1px solid var(--line);
  }
  .facts dt {
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .facts dd {
    margin: 0;
    font-size: 15px;
  }
  .line {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .line span {
    color: var(--text-2);
  }
  .cloud {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
    cursor: help;
  }
  .meta {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .meta span {
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    cursor: help;
  }
  .acts {
    display: flex;
    gap: 6px;
    margin-top: auto;
    padding-top: 2px;
  }
  .grow {
    flex: 1;
  }
  .pin path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  .pin.on path {
    fill: currentColor;
  }
</style>
