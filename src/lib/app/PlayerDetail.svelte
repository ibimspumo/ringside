<script lang="ts">
  import { api } from "../api";
  import PlayingCard from "../ui/PlayingCard.svelte";
  import { STAT_META, fmtUnit, stat, tone, toneColor, usual } from "../stats";
  import type { HudConfig, PlayerNote, PlayerView, ShownHand, StatId } from "../types";
  import Button from "../ui/Button.svelte";
  import EmptyState from "../ui/EmptyState.svelte";
  import IconButton from "../ui/IconButton.svelte";
  import SourceNote from "../ui/SourceNote.svelte";
  import StatMeter from "../ui/StatMeter.svelte";
  import Term from "../ui/Term.svelte";
  import { dateTime, int, sign, signed } from "./format";
  import { HINT, SMALL_SAMPLE, sampleNote } from "./reads";
  import Wheel from "./Wheel.svelte";

  let {
    player,
    hud,
    advice,
    hero = false,
    docked = false,
    onClose,
    onHands,
  }: {
    player: PlayerView;
    hud: HudConfig;
    /** Player type and counter may show: PokerStars is closed. */
    advice: boolean;
    /** One of your own names: no player type for yourself. */
    hero?: boolean;
    /** Next to the list instead of sliding in over it. */
    docked?: boolean;
    onClose?: () => void;
    onHands: (name: string, handId?: number) => void;
  } = $props();

  const GROUPS: [string, StatId[]][] = [
    ["Vor dem Flop", ["vpip", "pfr", "three_bet", "fold_to_three_bet", "steal", "fold_to_steal"]],
    ["Nach dem Flop", ["cbet", "fold_to_cbet", "wtsd", "wsd", "af", "afq"]],
  ];

  let shown = $state<ShownHand[] | null>(null);
  let note = $state<PlayerNote | null>(null);
  let panel = $state<HTMLElement>();

  $effect(() => {
    const name = player.name;
    let live = true;
    shown = null;
    note = null;
    api.shownHands(name).then((r) => live && (shown = r));
    api.playerNote(name).then((n) => live && (note = n));
    if (!docked) panel?.focus();
    return () => (live = false);
  });

  const hands = $derived(stat(player.stats, "hands")?.value ?? 0);
  const r = $derived(player.read);
  const cards = (s: string) => s.split(" ").filter(Boolean);

  /** Where the value sits: below, inside (lower or upper end) or above the usual range. */
  function word(id: StatId): string {
    const s = stat(player.stats, id);
    const t = tone(hud, s);
    if (t === "low") return "unter üblich";
    if (t === "high") return "über üblich";
    if (t !== "mid" || s?.value == null) return "noch kein Wert";
    const r = hud.thresholds[id];
    if (!r) return "üblich";
    const at = (s.value - r.low) / (r.high - r.low);
    return at >= 2 / 3 ? "üblich, oberes Ende" : at <= 1 / 3 ? "üblich, unteres Ende" : "üblich";
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && onClose && !e.defaultPrevented) onClose();
  }
</script>

<svelte:window onkeydown={docked ? undefined : key} />

{#if !docked}<div class="backdrop" onclick={onClose} aria-hidden="true"></div>{/if}
<aside class="detail" class:docked class:drawer={!docked} bind:this={panel} tabindex="-1" aria-label="Spieler {player.name}">
  <header>
    <Wheel stats={player.stats} {hud} size={44} />
    <div class="who">
      <h2 title={player.name}>{player.name}</h2>
      <span class="sub">
        <span class="num">{int(hands)}</span> gemeinsame {hands === 1 ? "Hand" : "Hände"}
      </span>
    </div>
    {#if onClose}
      <IconButton label="Schließen (Esc)" onclick={onClose}><svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 6l12 12M18 6 6 18" /></svg></IconButton>
    {/if}
  </header>

  <div class="body">
    {#if hero}
      <p class="self">Das bist du. Wie gut deine eigenen Werte sind, zeigt dir die Seite <b>Schwachstellen</b>.</p>
    {:else if !advice}
      <div class="lock"><SourceNote kind="locked" hint="Spielertyp und Gegenzug siehst du wieder, sobald du PokerStars schließt. Die Werte bleiben sichtbar." /></div>
    {:else if r?.type}
      {@const t = r.type}
      <section class="read" aria-label="Spielertyp">
        <div class="read-head">
          <span class="kicker">Spielertyp</span>
          <SourceNote kind="estimate" hint="Faustregel aus VPIP und PFR, wie im Lernmodul „Gegner lesen“." />
        </div>
        <p class="type"><b>{t.name}{#if t.long !== t.name}<small>· {t.long.slice(t.name.length).replace(/[()]/g, "").trim()}</small>{/if}</b> <span>{t.sketch}</span></p>
        <p class="counter"><span class="kicker">{r.sure === "sure" ? "Gegenzug" : "Gegenzug, vorläufig"}</span>{t.counter}</p>
        <p class="why">{r.sure === "sure" ? t.why : `Bisher: ${t.why}`}</p>
        <p class="sure" class:warn={r.sure !== "sure"}>
          <i aria-hidden="true"></i>
          {#if r.sure === "sure"}
            Klares Muster nach {int(r.hands)} Händen: Der Typ passt gut, ganz fest steht er aber nie.
          {:else if r.sure === "maybe"}
            Knapp nach {int(r.hands)} Händen: Er liegt nah an der Grenze zu einem anderen Typ.
          {:else}
            Noch unsicher nach {int(r.hands)} Händen: Mit mehr Händen kann sich der Typ noch ändern. Beobachte ihn weiter.
          {/if}
        </p>
      </section>
    {:else if r}
      <section class="read" aria-label="Spielertyp">
        <div class="read-head"><span class="kicker">Spielertyp</span></div>
        <p class="type"><b>Kein klarer Typ</b> <span>Seine Werte liegen zwischen den vier Typen.</span></p>
        <p class="why">Spiel gegen ihn solide und achte auf einzelne Werte, die weit vom Üblichen abweichen.</p>
      </section>
    {/if}

    {#if note && (note.text || note.label)}
      <section class="note" style="--c:{note.label?.color ?? 'var(--line-2)'}" aria-label="Deine Notiz">
        <span class="note-head">
          {#if note.label}<i aria-hidden="true"></i>{note.label.name || "Label"}{:else}Deine Notiz{/if}
          <small>deine Notiz aus PokerStars</small>
        </span>
        {#if note.text}<p>{note.text}</p>{/if}
      </section>
    {/if}

    {#each GROUPS as [title, ids] (title)}
      {@const unsure = ids.filter((id) => (stat(player.stats, id)?.opp ?? 0) < SMALL_SAMPLE).length}
      <section class="group" aria-label={title}>
        <h3>{title}</h3>
        {#if unsure}
          <p class="few"><i aria-hidden="true"></i>{unsure === ids.length ? "Alle Werte" : `${unsure} von ${ids.length} Werten`} stammen aus weniger als {SMALL_SAMPLE} Gelegenheiten: noch unsicher.</p>
        {/if}
        <dl>
          {#each ids as id (id)}
            {@const s = stat(player.stats, id)}
            {@const range = hud.thresholds[id]}
            {@const thin = (s?.opp ?? 0) < SMALL_SAMPLE}
            {@const t = tone(hud, s)}
            {@const hint = advice && !hero && !thin && (t === "low" || t === "high") ? HINT[id]?.[t] : undefined}
            <div class="stat" class:thin>
              <dt>
                <Term tip={`${STAT_META[id].name}: ${STAT_META[id].help}`} side="left">{STAT_META[id].name}</Term>
                <small>{s?.value == null ? "noch kein Wert" : `${word(id)} · ${sampleNote(s, true)}`}</small>
              </dt>
              <dd>
                {#if range}
                  <StatMeter
                    value={s?.value ?? null}
                    low={range.low}
                    high={range.high}
                    max={STAT_META[id].ratio ? 6 : 100}
                    color={toneColor(hud, s, "var(--text-3)")}
                    {thin}
                    label={`${STAT_META[id].name} ${fmtUnit(s)}, üblich ${usual(id, range)}`}
                  />
                {/if}
                <b class="num" style="color:{toneColor(hud, s, 'var(--text-3)')}">{fmtUnit(s)}</b>
              </dd>
              {#if hint}<dd class="hint">→ {hint}</dd>{/if}
            </div>
          {/each}
        </dl>
      </section>
    {/each}

    <section class="group" aria-label="Gezeigte Karten">
      <h3>Gezeigte Karten</h3>
      {#if shown === null}
        <EmptyState inline tone="loading" title="Lädt …" />
      {:else if shown.length === 0}
        <EmptyState inline title="Noch keine gezeigten Karten" text="Sobald {player.name} am Showdown Karten zeigt, stehen sie hier." />
      {:else}
        <ul class="shown">
          {#each shown as h (h.hand_id)}
            {@const net = h.big_blind ? h.net / h.big_blind : 0}
            <li>
              <span class="hole">{#each cards(h.cards) as c (c)}<PlayingCard card={c} size={24} />{/each}</span>
              <span class="board">
                {#each cards(h.board) as c (c)}<PlayingCard card={c} size={18} />{/each}
                {#if !h.board}<span class="none">kein Board</span>{/if}
              </span>
              <span class="meta">
                <b class="num {sign(net)}">{signed(net)}<span class="unit">BB</span></b>
                <small>{h.position ?? "–"} · {dateTime(h.played_at)}</small>
              </span>
              <Button size="sm" variant="quiet" onclick={() => onHands(player.name, h.hand_id)}>Abspielen</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>

  <footer>
    <Button full onclick={() => onHands(player.name)}>Alle Hände mit {player.name}</Button>
  </footer>
</aside>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: var(--z-dock);
    background: var(--overlay);
    animation: fade var(--dur) var(--ease-standard);
  }
  .detail {
    display: flex;
    flex-direction: column;
    background: var(--surface);
    outline: none;
  }
  .drawer {
    position: fixed;
    z-index: var(--z-dock);
    top: 0;
    right: 0;
    bottom: 0;
    width: min(460px, calc(100vw - 220px));
    border-left: 1px solid var(--line-2);
    box-shadow: var(--shadow-3);
    animation: slide var(--dur-slow) var(--ease-enter);
  }
  .docked {
    position: sticky;
    top: var(--sp-4);
    max-height: calc(100vh - 140px);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    overflow: hidden;
  }
  @keyframes slide {
    from {
      transform: translateX(32px);
      opacity: 0;
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-4) var(--sp-4) var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--line);
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-family: var(--font-num);
    font-size: var(--fs-xl);
    font-weight: var(--fw-semibold);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .note-head i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  /* The fade at the bottom says the list goes on under the footer. */
  .body {
    flex: 1;
    overflow: auto;
    mask-image: linear-gradient(to bottom, var(--surface) calc(100% - 28px), transparent);
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    padding: var(--sp-4) var(--sp-5) var(--sp-8);
  }
  .kicker {
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .read {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-4);
    border: 1px solid var(--line-2);
    border-radius: var(--r-md);
    background: var(--surface-2);
  }
  .read-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2);
  }
  .read p {
    margin: 0;
  }
  .type b {
    display: block;
    font-family: var(--font-num);
    font-size: var(--fs-2xl);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .type small {
    margin-left: var(--sp-2);
    color: var(--text-2);
    font-family: var(--font-ui);
    font-size: var(--fs-lg);
    font-weight: var(--fw-regular);
  }
  .type span {
    color: var(--text-2);
  }
  .read .counter {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: var(--sp-1);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
    font-size: var(--fs-lg);
    font-weight: var(--fw-medium);
  }
  .why {
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
  }
  .sure {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .sure i {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--pos);
    transform: translateY(-1px);
  }
  .sure.warn i {
    background: none;
    box-shadow: inset 0 0 0 2px var(--amber);
  }
  .lock {
    align-self: flex-start;
  }
  .self {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .note {
    padding: var(--sp-3);
    border-radius: var(--r-sm);
    border-left: 3px solid var(--c);
    background: var(--surface-2);
    font-size: var(--fs-md);
  }
  .note-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-weight: var(--fw-medium);
  }
  .note-head small {
    margin-left: auto;
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-regular);
  }
  .note p {
    margin: var(--sp-1) 0 0;
    color: var(--text-2);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  h3 {
    margin: 0 0 var(--sp-1);
    color: var(--text-3);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  dl {
    margin: 0;
  }
  .few {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    margin: 0 0 var(--sp-1);
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .few i {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 2px var(--text-2);
  }
  .stat {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(120px, 34%);
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--line);
  }
  .stat:last-child {
    border-bottom: 0;
  }
  dt {
    min-width: 0;
    font-size: var(--fs-base);
  }
  dt small {
    display: block;
    margin-top: 1px;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  dd {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 52px;
    align-items: center;
    gap: var(--sp-3);
    margin: 0;
  }
  dd b {
    font-size: var(--fs-lg);
    font-weight: var(--fw-semibold);
    text-align: right;
  }
  dd.hint {
    display: block;
    grid-column: 1 / -1;
    margin-top: calc(-1 * var(--sp-2));
    color: var(--text);
    font-size: var(--fs-md);
  }
  .shown {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .shown li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--line);
  }
  .shown li:last-child {
    border-bottom: 0;
  }
  .hole,
  .board {
    display: flex;
    gap: 3px;
  }
  .board {
    gap: 2px;
  }
  .none {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .meta b {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
  }
  .meta small {
    color: var(--text-3);
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  footer {
    padding: var(--sp-3) var(--sp-5);
    border-top: 1px solid var(--line);
  }
  @media (prefers-reduced-motion: reduce) {
    .drawer,
    .backdrop {
      animation: none;
    }
  }
</style>
