<script lang="ts">
  import Logo from "../Logo.svelte";
  import ActionChain from "./ActionChain.svelte";
  import ActionLabel from "./ActionLabel.svelte";
  import Badge from "./Badge.svelte";
  import BestHand from "./BestHand.svelte";
  import Button from "./Button.svelte";
  import Card from "./Card.svelte";
  import CardText from "./CardText.svelte";
  import Dialog from "./Dialog.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { rectOf, type Anchor } from "./floating";
  import { bb, cash, int, money, pct, signed } from "./format";
  import GradeBadge from "./GradeBadge.svelte";
  import GradeBars from "./GradeBars.svelte";
  import ColumnBars from "./ColumnBars.svelte";
  import IconButton from "./IconButton.svelte";
  import Kbd from "./Kbd.svelte";
  import MiniCard from "./MiniCard.svelte";
  import NavItem from "./NavItem.svelte";
  import PageHeader from "./PageHeader.svelte";
  import Panel from "./Panel.svelte";
  import PlayingCard from "./PlayingCard.svelte";
  import PokerTable, { COMPACT_BELOW, type TableSeat } from "./PokerTable.svelte";
  import PotOdds from "./PotOdds.svelte";
  import Popover from "./Popover.svelte";
  import { ACTION_NAME, GRADE, GRADES, SOURCE, type ActionKind, type Source } from "./poker";
  import ProgressBar from "./ProgressBar.svelte";
  import ProgressRing from "./ProgressRing.svelte";
  import RangeGrid, { type RangeCell } from "./RangeGrid.svelte";
  import SectionHead from "./SectionHead.svelte";
  import ChoiceMenu from "./ChoiceMenu.svelte";
  import SegmentedControl from "./SegmentedControl.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Slider from "./Slider.svelte";
  import SourceNote from "./SourceNote.svelte";
  import Stat from "./Stat.svelte";
  import StatMeter from "./StatMeter.svelte";
  import SearchField from "./SearchField.svelte";
  import SortHeader from "./SortHeader.svelte";
  import CellButton from "./CellButton.svelte";
  import Switch from "./Switch.svelte";
  import Tabs from "./Tabs.svelte";
  import CardSlot from "./CardSlot.svelte";
  import MixBar from "./MixBar.svelte";
  import Tag from "./Tag.svelte";
  import Toast from "./Toast.svelte";
  import Term from "./Term.svelte";
  import Tooltip from "./Tooltip.svelte";
  import Verdict from "./Verdict.svelte";

  /**
   * The kit catalogue (?view=kit): every building block in its variants and states. Design loops
   * compare their area against it. `&only=<id>` shows one section, `&zoom=<n>` magnifies it,
   * `&gray=1` drops colour to check that meaning survives without it.
   */
  const q = new URLSearchParams(window.location.search);
  const only = q.get("only");
  const zoom = Number(q.get("zoom")) || 1;
  const gray = q.get("gray") === "1";

  $effect(() => {
    document.body.classList.add("app");
    document.title = "Ringside · Kit";
  });

  const SECTIONS = [
    ["seitenkopf", "Seitenkopf"],
    ["grundlagen", "Grundlagen"],
    ["bewegung", "Bewegung"],
    ["knoepfe", "Knöpfe & Kürzel"],
    ["auswahl", "Auswahl"],
    ["navigation", "Navigation"],
    ["flaechen", "Flächen & Kennzahlen"],
    ["einstellungen", "Einstellungen"],
    ["schilder", "Schilder"],
    ["bewertung", "Bewertung"],
    ["karten", "Karten"],
    ["tisch", "Tisch"],
    ["raster", "Range-Raster"],
    ["popups", "Hinweise & Popups"],
    ["zustaende", "Fortschritt & Zustände"],
  ] as const;
  const show = (id: string) => !only || only === id;
  const NAV_ICON = {
    learn: "M12 5 3.5 9l8.5 4 8.5-4zM7 10.7v4.3c0 1.3 2.2 2.8 5 2.8s5-1.5 5-2.8v-4.3M20.5 9v5",
    play: "M12 5.5c4.4 0 8 2.9 8 6.5s-3.6 6.5-8 6.5-8-2.9-8-6.5 3.6-6.5 8-6.5zM9 10h2.6v4H9zM12.4 10H15v4h-2.6z",
    leak: "M12 4.5c3 4 5.5 6.9 5.5 10a5.5 5.5 0 0 1-11 0c0-3.1 2.5-6 5.5-10z",
    live: "M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5z",
    stats: "M5 19.5V13M10 19.5V8.5M15 19.5v-8M20 19.5V4.5M3.5 19.5h17",
  };

  // Einstellungen
  let autoOn = $state(true);
  let shareOn = $state(false);
  let libMode = $state("idle");
  let jumpTo = $state("haende");

  // Seitenkopf
  let demoTab = $state("setup");
  let demoPeriod = $state("30d");

  // Grundlagen
  const SURFACES = ["--bg", "--surface", "--surface-2", "--surface-3", "--line", "--line-2"];
  const TEXTS = ["--text", "--text-2", "--text-3"];
  const ROLES = ["--primary", "--secondary", "--danger", "--success", "--info", "--amber", "--source", "--focus"];
  const ACTIONS = ["--act-fold", "--act-check", "--act-bet-1", "--act-bet-2", "--act-bet-3", "--act-bet-4", "--act-allin", "--act-limp"];
  const TYPE = [
    ["--fs-hero", "44", "Große Kennzahl"],
    ["--fs-4xl", "34", "Kennzahl groß"],
    ["--fs-3xl", "26", "Seitentitel"],
    ["--fs-2xl", "22", "Kennzahl"],
    ["--fs-xl", "18", "Abschnitt"],
    ["--fs-lg", "16", "Kartentitel"],
    ["--fs-base", "14", "Fließtext"],
    ["--fs-md", "13", "Bedienung, Tabellen"],
    ["--fs-sm", "12", "Nebentext, Schilder"],
    ["--fs-xs", "11", "Kleinste Beschriftung"],
  ];
  const SPACE = [1, 2, 3, 4, 5, 6, 8, 10, 12];
  const RADII = ["--r-xs", "--r-sm", "--r-md", "--r", "--r-xl", "--r-pill"];

  // Bewegung
  const CURVES = [
    ["--ease-standard", "Standard", "Zustandswechsel: Hover, Auswahl, Füllstände"],
    ["--ease-enter", "Eintritt", "Was erscheint: Popover, Dialog, Toast, Karte aufdecken"],
    ["--ease-exit", "Austritt", "Was geht: schließen, ausblenden"],
    ["--ease-spring", "Federn", "Kleine Bestätigung: Unterstrich, hervorgehobene Karte"],
  ];
  const DURS = [
    ["--dur-fast", "120 ms"],
    ["--dur", "200 ms"],
    ["--dur-slow", "320 ms"],
  ];
  let played = $state(false);
  let replay = $state(0);
  let ringValue = $state(0.35);

  // Knöpfe
  const VARIANTS = ["primary", "secondary", "quiet", "danger", "danger-quiet"] as const;
  const VNAME = { primary: "Haupt", secondary: "Neben", quiet: "Leise", danger: "Gefahr", "danger-quiet": "Gefahr leise" };
  const SIZES = ["sm", "md", "lg"] as const;
  const STATES = [
    ["Ruhe", {}],
    ["Hover", { "data-force": "hover" }],
    ["Fokus", { "data-force": "focus" }],
    ["Deaktiviert", { disabled: true }],
    ["Lädt", { loading: true }],
  ] as const;

  // Auswahl
  let period = $state("7d");
  let small = $state("bb");
  const DEPTHS = [2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 20, 25, 30, 40, 50, 75, 100];
  let depth = $state(25);
  let tab = $state("hands");
  let tags = $state(["BTN", "CO"]);
  let picked = $state<string[]>(["Cash"]);
  const toggle = (t: string) => (picked = picked.includes(t) ? picked.filter((x) => x !== t) : [...picked, t]);

  // Schilder
  const KINDS: ActionKind[] = ["fold", "check", "call", "bet", "raise", "allin"];
  const SOURCES = Object.keys(SOURCE) as Source[];

  // Tisch
  const SIX: TableSeat[] = [
    { position: "BB", stack: 44.2, cards: ["Th", "8h"], action: { kind: "call" }, bet: 6 },
    { position: "UTG", name: "maverick_77", stack: 70, folded: true },
    { position: "HJ", name: "nordlicht", stack: 70, folded: true },
    { position: "CO", name: "Kiebitz", stack: 70, folded: true },
    { position: "BTN", name: "riverrat", stack: 70, folded: true, dealer: true },
    { position: "SB", name: "Luna_K", stack: 60.5, cards: "hidden", action: { kind: "bet", amount: 6, step: 2 }, bet: 6, toAct: false },
  ];
  const REPLAY: TableSeat[] = [
    { position: "SB", stack: 51.7, cards: ["9s", "9d"], action: { kind: "call" }, grade: "blunder" },
    { position: "BB", name: "Eve", stack: 97, folded: true },
    { position: "LJ", name: "Alice_im_Wunderland", stack: 76, cards: ["As", "Kc"], won: 157 },
    { position: "HJ", name: "Bob", stack: 114, folded: true },
    { position: "CO", name: "Carol", stack: 80, folded: true },
    { position: "BTN", name: "Dave", stack: 99.5, folded: true, dealer: true },
  ];
  const NINE: TableSeat[] = [
    { position: "BTN", stack: 98, cards: ["As", "Kd"], action: { kind: "raise", amount: 2.5, step: 2 }, bet: 2.5, dealer: true },
    { position: "SB", name: "Luna_K", stack: 99.5, folded: true, bet: 0.5 },
    { position: "BB", name: "Kiebitz", stack: 97, cards: "hidden", action: { kind: "raise", amount: 9, step: 3 }, bet: 9, toAct: false },
    { position: "UTG", name: "maverick_77", stack: 100, folded: true },
    { position: "UTG+1", name: "nordlicht", stack: 100, folded: true },
    { empty: true },
    { position: "LJ", name: "Gerd", stack: 31, cards: "hidden", action: { kind: "allin", amount: 31 }, bet: 31, toAct: true },
    { position: "HJ", name: "fishbowl", stack: 100, folded: true },
    { position: "CO", name: "riverrat", stack: 100, folded: true },
  ];

  // Raster
  const mixCell = (i: number): RangeCell => {
    const r = Math.floor(i / 13);
    const c = i % 13;
    const strength = 1 - (r + c) / 24;
    if (strength < 0.18) return null;
    const raise = Math.max(0, Math.min(1, (strength - 0.45) * 2.2));
    const call = Math.max(0, Math.min(1 - raise, 0.6 - Math.abs(strength - 0.5)));
    return {
      mix: [
        { color: "var(--act-bet-3)", share: raise },
        { color: "var(--act-call)", share: call },
        { color: "var(--act-fold)", share: Math.max(0, 1 - raise - call) },
      ],
      weight: Math.min(1, 0.25 + strength),
      note: `Raise ${pct(raise)}, Call ${pct(call)}`,
    };
  };
  const strategy = Array.from({ length: 169 }, (_, i) => mixCell(i));
  let gridSel = $state(-1);
  const FILLS = [0, 0.25, 0.5, 0.75, 1];
  const SPEC_COLORS = ["--act-fold", "--act-check", "--act-bet-1", "--act-bet-2", "--act-bet-3", "--act-bet-4", "--act-allin", "--pos", "--neg", "--sd"];
  // Specimen k uses class index k, so every specimen keeps its own label.
  const specimens: RangeCell[] = Array.from({ length: 169 }, (_, k) => {
    const color = SPEC_COLORS[Math.floor(k / FILLS.length)];
    const weight = FILLS[k % FILLS.length];
    return color && weight > 0 ? { color: `var(${color})`, weight } : color ? { color: "transparent", weight: 0 } : null;
  });
  const specimenIdx = Array.from({ length: SPEC_COLORS.length * FILLS.length }, (_, k) => k);
  const stateCells: RangeCell[] = Array.from({ length: 169 }, () => null);
  stateCells[0] = { color: "var(--act-bet-1)", weight: 1 };
  stateCells[1] = { color: "var(--act-bet-1)", weight: 0.5 };
  stateCells[2] = { mix: [{ color: "var(--act-bet-1)", share: 0.4 }, { color: "var(--act-call)", share: 0.35 }, { color: "var(--act-fold)", share: 0.25 }], weight: 0.8 };
  stateCells[3] = { mix: [{ color: "var(--act-allin)", share: 0.5 }, { color: "var(--act-fold)", share: 0.5 }] };
  stateCells[4] = { color: "var(--act-bet-1)", weight: 1 };
  stateCells[5] = null;
  stateCells[6] = { color: "var(--pos)", weight: 1, value: "+2,4" };
  const STATE_LABEL = ["voll", "halb", "gemischt", "All-in-Mix", "abgeblendet", "nicht drin", "mit Wert"];

  // Popups
  let dialogOpen = $state(false);
  let popAnchor = $state<Anchor | null>(null);
  let popHost = $state<HTMLElement>();
  $effect(() => {
    if (popHost && (only === "popups" || q.get("pop") === "1")) popAnchor = rectOf(popHost);
  });
  let kitSize = $state(6);
  let kitSearch = $state("");
  let kitSort = $state<"name" | "hands">("hands");
</script>

<div class="kit" class:gray>
  <aside class="nav" aria-label="Abschnitte">
    <div class="brand"><Logo size={26} /><span><b>Ringside</b> Kit</span></div>
    <nav>
      {#each SECTIONS as [id, name] (id)}<a href="?view=kit#{id}" class:on={only === id}>{name}</a>{/each}
    </nav>
    <p class="hint">Jeder Baustein in allen Varianten und Zuständen. Design-Loops vergleichen ihren Bereich mit diesem Bild.</p>
  </aside>

  <main style:zoom={zoom !== 1 ? zoom : undefined}>
    {#if !only}
      <div class="kit-head">
        <PageHeader title="Bausteine" sub="Alles in src/lib/ui/. Eine Hauptaktion je Ansicht, Zahlen mit Bedeutung, Pokerbegriffe erklärt." />
      </div>
    {/if}

    {#snippet headEnd()}
      <ProgressRing size={20} stroke={3} tone="good" value={0.7} label="Fortschritt" />
      <span class="muted">211 Flop-Spots rechnen noch</span>
      <Button size="sm" variant="quiet">Filter</Button>
    {/snippet}
    {#if show("seitenkopf")}
      <section id="seitenkopf">
        <SectionHead
          title="Seitenkopf"
          sub="Die einzige Überschrift jeder Seite: immer oben, immer gleich hoch. Bereiche einer Seite stehen als eigene Reiterzeile darunter, seitenweite Filter rechts."
        />
        <div class="frame">
          <PageHeader
            demo
            title="Solver"
            sub="Heads-up ab Flop, Turn oder River: Spot aufbauen, lösen, durchspielen und üben."
            sections={{
              value: demoTab,
              onchange: (id) => (demoTab = id),
              tabs: [
                { id: "setup", label: "Aufbau" },
                { id: "result", label: "Durchspielen" },
                { id: "practice", label: "Üben" },
                { id: "saved", label: "Meine Lösungen", disabled: true, hint: "Erst lösen" },
              ],
            }}
          />
        </div>
        <div class="frame">
          <PageHeader
            demo
            title="Hände"
            sections={{
              value: "mistakes",
              tabs: [
                { id: "mistakes", label: "Meine teuersten Fehler" },
                { id: "all", label: "Alle Hände" },
              ],
              end: headEnd,
            }}
          />
        </div>
        <div class="frame">
          <PageHeader demo title="Übersicht" sub="Wie läuft es – und was solltest du als Nächstes üben?">
            {#snippet actions()}
              <SegmentedControl label="Spielart" size="sm" value="all" options={[{ value: "all", label: "Alle" }, { value: "cash", label: "Cash" }, { value: "mtt", label: "Turnier" }]} />
              <SegmentedControl label="Zeitraum" size="sm" bind:value={demoPeriod} options={[{ value: "7d", label: "7 Tage" }, { value: "30d", label: "30 Tage" }, { value: "all", label: "Alles" }]} />
            {/snippet}
          </PageHeader>
        </div>
        <div class="frame wide-frame">
          <PageHeader demo title="Coach" sub="Breites Fenster: Kopf und Inhalt teilen die linke und rechte Kante." frame={{ left: 330, right: 330 }} />
          <div class="frame-page"><Card tone="sunken"><span class="muted">Inhalt der Seite (.page)</span></Card></div>
        </div>
        <p class="muted">Der Kopf folgt Breite und Rand der Seite darunter (<code>use:followPage</code> am Inhaltsbereich); ohne Seite läuft er über die volle Breite.</p>
        <p class="muted">Seiten melden Zeile und Bereiche mit <code>usePageHead(() =&gt; ({"{"} sub, tabs, value, onchange, end {"}"}))</code> an; <code>end</code> sind leise Steuerungen am Ende der Reiterzeile. Ein eigenes <code>&lt;h1&gt;</code> meldet die Prüfung.</p>
      </section>
    {/if}

    {#if show("grundlagen")}
      <section id="grundlagen">
        <SectionHead title="Grundlagen" sub="Farben nach Rolle, eine Schriftskala, Abstände im 4er-Raster." />
        <div class="swatches">
          {#each [["Flächen & Linien", SURFACES], ["Text", TEXTS], ["Rollen", ROLES], ["Poker-Aktionen", ACTIONS]] as [title, list] (title)}
            <div class="swatch-group">
              <h3>{title}</h3>
              {#each list as v (v)}
                <div class="swatch"><i style:background="var({v})"></i><code>{v}</code></div>
              {/each}
            </div>
          {/each}
          <div class="swatch-group">
            <h3>Bewertung</h3>
            {#each GRADES as g (g)}
              <div class="swatch"><i style:background="var(--grade-{g})"></i><code>--grade-{g}</code></div>
            {/each}
          </div>
        </div>
        <div class="two">
          <Panel title="Schriftstufen" sub="Nur diese Größen; die Prüfung meldet freie Werte.">
            <ul class="type">
              {#each TYPE as [v, px, use] (v)}
                <li><span style:font-size="var({v})" class:num={Number(px) >= 22}>{Number(px) >= 22 ? "12,4 BB" : "Nächste Hand"}</span><code>{v} · {px}</code><em>{use}</em></li>
              {/each}
            </ul>
          </Panel>
          <div class="stack">
            <Panel title="Abstände" sub="--sp-1 = 4 px, alles ein Vielfaches davon.">
              <ul class="space">
                {#each SPACE as n (n)}<li><i style:width="var(--sp-{n})"></i><code>--sp-{n}</code><span class="num">{n * 4}</span></li>{/each}
              </ul>
            </Panel>
            <Panel title="Radien & Schatten">
              <div class="radii">
                {#each RADII as r (r)}<span style:border-radius="var({r})"><code>{r}</code></span>{/each}
              </div>
              <div class="shadows">
                {#each [1, 2, 3] as s (s)}<span style:box-shadow="var(--shadow-{s})"><code>--shadow-{s}</code></span>{/each}
              </div>
            </Panel>
            <Panel title="Zahlenformat" sub="src/lib/ui/format.ts – überall dieselben Helfer.">
              <dl class="formats">
                <dt><code>bb(1.15)</code></dt><dd class="num">{bb(1.15)}</dd>
                <dt><code>bb(-3, {"{"} sign {"}"})</code></dt><dd class="num">{bb(-3, { sign: true })}</dd>
                <dt><code>pct(0.123)</code></dt><dd class="num">{pct(0.123)}</dd>
                <dt><code>signed(12.4)</code></dt><dd class="num">{signed(12.4)}</dd>
                <dt><code>int(12480)</code></dt><dd class="num">{int(12480)}</dd>
                <dt><code>money(-1140, "USD")</code></dt><dd class="num">{money(-1140, "USD")}</dd>
                <dt><code>cash(550, "EUR")</code></dt><dd class="num">{cash(550, "EUR")}</dd>
              </dl>
            </Panel>
          </div>
        </div>
      </section>
    {/if}

    {#if show("bewegung")}
      <section id="bewegung">
        <SectionHead title="Bewegung" sub="Kurven statt linear. Bei „Bewegung reduzieren“ im System steht alles still.">
          {#snippet actions()}
            <Button variant="primary" kbd="A" onclick={() => ((played = !played), replay++, (ringValue = ringValue > 0.5 ? 0.2 : 0.85))}>Alles abspielen</Button>
          {/snippet}
        </SectionHead>
        <div class="curves">
          {#each CURVES as [v, name, use] (v)}
            <div class="curve">
              <div class="track"><i class:go={played} style:transition-timing-function="var({v})"></i></div>
              <b>{name}</b><code>{v}</code><span>{use}</span>
            </div>
          {/each}
          <div class="durs">
            {#each DURS as [v, ms] (v)}<span><code>{v}</code> {ms}</span>{/each}
          </div>
        </div>
        <div class="motion-grid">
          {#key replay}
            <Card pad="md"><h4>Karte aufdecken</h4><div class="row">{#each ["Ah", "Kd", "7c"] as c, k (c)}<PlayingCard card={c} size={40} reveal delay={k * 90} />{/each}</div></Card>
            <Card pad="md"><h4>Toast</h4><Toast tone="good" title="Analyse fertig" text="12 Hände bewertet" /></Card>
            <Card pad="md"><h4>Beste Hand</h4><BestHand hole={["Jh", "Tc"]} board={["9c", "8c", "7c", "2d", "Ks"]} winner reveal size={30} /></Card>
          {/key}
          <Card pad="md">
            <h4>Fortschritt</h4>
            <div class="row"><ProgressRing value={ringValue} label="Fortschritt" size={48}>{Math.round(ringValue * 100)}</ProgressRing><div class="grow"><ProgressBar value={ringValue} label="Fortschritt" size="md" /></div></div>
          </Card>
          <Card pad="md"><h4>Reiter & Segmente</h4><Tabs label="Beispiel" bind:value={tab} tabs={[{ id: "hands", label: "Hände" }, { id: "stats", label: "Statistik" }, { id: "leaks", label: "Leaks", note: "nach dem Lösen" }]} /><div class="gap"></div><SegmentedControl label="Zeitraum" bind:value={period} options={[{ value: "today", label: "Heute" }, { value: "7d", label: "7 Tage" }, { value: "all", label: "Alles" }]} /></Card>
          <Card pad="md"><h4>Dialog</h4><p class="muted">Öffnet mit Eintrittskurve, Hintergrund blendet ab.</p><Button onclick={() => (dialogOpen = true)}>Dialog öffnen</Button></Card>
        </div>
      </section>
    {/if}

    {#if show("knoepfe") || only === "kbd"}
      <section id="knoepfe">
        {#if only !== "kbd"}
        <SectionHead title="Knöpfe & Kürzel" sub="Haupt = der eine nächste Schritt. Neben = Alternativen. Leise = Beiläufiges. Gefahr = löscht etwas." />
        <div class="matrix-table" style:--cols={STATES.length}>
          <span></span>
          {#each STATES as [name] (name)}<span class="col-head">{name}</span>{/each}
          {#each VARIANTS as v (v)}
            <span class="row-head">{VNAME[v]}</span>
            {#each STATES as [name, props] (name)}
              <span><Button variant={v} {...props}>{v === "danger" || v === "danger-quiet" ? "Löschen" : v === "primary" ? "Nächste Hand" : v === "quiet" ? "Abbrechen" : "Wiederholen"}</Button></span>
            {/each}
          {/each}
        </div>
        <h3 class="sub-head">Größen mit Tastenkürzel</h3>
        {/if}
        <div class="kbd-sizes">
          {#each SIZES as s (s)}
            <div class="row">
              <Button size={s} variant="primary" kbd="Leertaste">Nächste Hand</Button>
              <Button size={s} kbd="R">Hand wiederholen</Button>
              <Button size={s} variant="quiet" kbd="Esc">Schließen</Button>
              <Button size={s} kbd="Strg + K">Suchen</Button>
              <Button size={s} kbd={["←"]}>Zurück</Button>
            </div>
          {/each}
          <p class="muted">Im Text: Drück <Kbd keys="1" />, <Kbd keys="2" /> oder <Kbd keys="Leertaste" />; vor- und zurückspulen mit <Kbd keys="←" /> <Kbd keys="→" />, suchen mit <Kbd keys="Strg + K" />.</p>
        </div>
        {#if only !== "kbd"}
        <h3 class="sub-head">Symbolknöpfe</h3>
        <div class="row">
          {#each SIZES as s (s)}
            <IconButton label="Einstellungen" size={s}><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3" /><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1" /></svg></IconButton>
          {/each}
          <IconButton label="Angeheftet" pressed><svg viewBox="0 0 24 24"><path d="M9 4h6l-1 6 3 3H7l3-3z" /><path d="M12 13v7" /></svg></IconButton>
          <IconButton label="Hover" data-force="hover"><svg viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg></IconButton>
          <IconButton label="Fokus" data-force="focus" variant="secondary"><svg viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg></IconButton>
          <IconButton label="Deaktiviert" disabled><svg viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg></IconButton>
        </div>
        {/if}
      </section>
    {/if}

    {#if show("auswahl")}
      <section id="auswahl">
        <SectionHead
          title="Auswahl"
          sub="Segmente für wenige gleichrangige Optionen, das Auswahlmenü für dieselbe Wahl in schmalen Kopfzeilen, Reiter für Bereiche einer Seite, Tags für Filter, der Regler für eine Zahl aus vielen Stufen."
        />
        <Card pad="md">
          <Slider label="Dein Stack" unit="BB" log values={DEPTHS} value={depth} onchange={(v) => (depth = v)} steps={[5, 10, 15, 20, 25, 30, 40, 50, 75, 100]} />
        </Card>
        <div class="gap"></div>
        <div class="two">
          <div class="stack">
            <SegmentedControl label="Zeitraum" bind:value={period} options={[{ value: "today", label: "Heute" }, { value: "7d", label: "7 Tage" }, { value: "30d", label: "30 Tage" }, { value: "all", label: "Alles" }]} />
            <SegmentedControl label="Einheit" size="sm" bind:value={small} options={[{ value: "bb", label: "BB" }, { value: "money", label: "Geld" }, { value: "pct", label: "% vom Pot", disabled: true }]} />
            <div class="row">
              <ChoiceMenu label="Zeitraum" value={period} onchange={(v) => (period = v)} options={[["today", "Heute"], ["7d", "7 Tage"], ["30d", "30 Tage"], ["all", "Alles"]]} />
              <ChoiceMenu label="Spielart" value={undefined} dim title="Gilt nicht, solange ein Turnier gewählt ist." onchange={() => {}} options={[["all", "Alle"], ["cash", "Cash"]]} />
            </div>
            <Tabs label="Bereiche" bind:value={tab} tabs={[{ id: "hands", label: "Hände", count: 248 }, { id: "stats", label: "Statistik" }, { id: "leaks", label: "Leaks", count: 3 }]} />
          </div>
          <div class="stack">
            <SearchField label="Spieler suchen" bind:value={kitSearch} />
            <div class="row">{#each ["Cash", "Turnier", "Spin & Go"] as t (t)}<Tag selected={picked.includes(t)} onclick={() => toggle(t)}>{t}</Tag>{/each}</div>
            <div class="row">{#each tags as t (t)}<Tag onremove={() => (tags = tags.filter((x) => x !== t))} removeLabel="Filter {t} entfernen">Position: {t}</Tag>{/each}<Tag>nur Beschriftung</Tag></div>
          </div>
        </div>
      </section>
    {/if}

    {#if show("navigation")}
      <section id="navigation">
        <SectionHead
          title="Navigation"
          sub="Einträge der oberen Leiste: Reiter mit Marke, die Zeile darunter und Kacheln im Menü mit Satz und Live-Wert. Die offene Seite ist unterstrichen."
        />
        <div class="stack">
          <div class="navbar">
            <NavItem size="top" label="Start" icon={NAV_ICON.live} />
            <NavItem size="top" label="Mein Spiel" icon={NAV_ICON.stats} chevron="down" current status="4 Schwachstellen" tone="warn" aria-expanded={false} />
            <NavItem size="top" label="Üben" icon={NAV_ICON.learn} chevron="down" locked aria-expanded={true} />
            <NavItem size="top" label="Gegner" icon={NAV_ICON.live} chevron="down" status="3" tone="good" aria-expanded={false} />
          </div>
          <div class="navbar">
            <NavItem size="sub" label="Hände" icon={NAV_ICON.play} status="rechnet 96 %" />
            <NavItem size="sub" label="Schwachstellen" icon={NAV_ICON.leak} status="4" tone="warn" active />
            <NavItem size="sub" label="Nachrechnen" icon={NAV_ICON.stats} locked />
          </div>
          <div class="navtiles">
            <NavItem size="tile" label="Schwachstellen" icon={NAV_ICON.leak} text="Wo du immer wieder Chips verlierst und was dagegen hilft." status="4 gefunden" tone="warn" active />
            <NavItem size="tile" label="Nachrechnen" icon={NAV_ICON.stats} text="Eine Situation genau durchrechnen lassen." status="gesperrt" locked />
          </div>
        </div>
      </section>
    {/if}

    {#if show("flaechen")}
      <section id="flaechen">
        <SectionHead title="Flächen & Kennzahlen" sub="Jede Zahl mit Bedeutung: gut oder schlecht, und warum." />
        <div class="three">
          <Card><Stat label="Gewinn" value="+12,4" unit="BB/100" tone="good" meaning="Gut: über dem Schnitt deines Limits." help="Gewinn je 100 Hände in Big Blinds" /></Card>
          <Card><Stat label="VPIP" value="38" unit="%" tone="warn" meaning="Zu viele Hände – Ziel 22–28 %." help="Wie oft du freiwillig Geld in den Pot legst" /></Card>
          <Card><Stat label="Kostet im Schnitt" value="−4,2" unit="BB" tone="bad" meaning="Meist am River: zu oft gecallt." /></Card>
          <Card tone="raised"><Stat size="lg" label="Genauigkeit" value="84" unit="%" meaning="12 von 14 Entscheidungen richtig." /></Card>
          <Card onclick={() => {}}><SectionHead level={3} title="Klickbare Karte" sub="Ganze Fläche ist ein Knopf." /></Card>
          <Card onclick={() => {}} selected><SectionHead level={3} title="Ausgewählt" sub="Rahmen in Textfarbe." /></Card>
          <Card onclick={() => {}} disabled title="Gerade nicht wählbar"><SectionHead level={3} title="Gesperrt" sub="Klickbar, aber gerade nicht wählbar." /></Card>
        </div>
        <div class="tiles">
          <span class="muted">Kacheln im Raster (<code>pad="xs"</code>):</span>
          <Card pad="xs" onclick={() => {}}><span class="tile">{#each ["Ah", "Kd", "7c"] as c (c)}<MiniCard card={c} />{/each}</span></Card>
          <Card pad="xs" onclick={() => {}} disabled><span class="tile">{#each ["Qs", "Qh", "5d"] as c (c)}<MiniCard card={c} />{/each}</span></Card>
        </div>
        <div class="two">
          <Panel title="Wert auf seiner Skala" sub="Band = üblicher Bereich, Punkt = Wert in HUD-Farbe; hohl heißt: zu wenig Gelegenheiten.">
            <div class="meters">
              <span>VPIP 33 %</span><StatMeter value={33} low={20} high={35} color="var(--amber)" label="VPIP 33 %, üblich 20–35 %" />
              <span>PFR 3 %</span><StatMeter value={3} low={12} high={25} color="var(--neg)" label="PFR 3 %, unter üblich" />
              <span>AF 4,2×</span><StatMeter value={4.2} max={6} low={1.5} high={3.5} color="var(--pos)" label="AF 4,2, über üblich" />
              <span>3-Bet 8 %</span><StatMeter value={8} low={4} high={10} color="var(--amber)" thin label="3-Bet 8 %, noch unsicher" />
              <span>klein</span><StatMeter size="sm" value={41} low={20} high={35} color="var(--pos)" label="VPIP 41 %" />
            </div>
          </Panel>
          <Panel title="Tabelle mit Sortierung" sub="Spaltentitel sortieren, die erste Zelle öffnet die Zeile – auch per Tastatur." flush>
            <table class="kit-table">
              <thead><tr><th><SortHeader active={kitSort === "name"} asc onclick={() => (kitSort = "name")}>Spieler</SortHeader></th><th class="r"><SortHeader active={kitSort === "hands"} onclick={() => (kitSort = "hands")}>Hände</SortHeader></th><th class="r"><SortHeader>Stack</SortHeader></th></tr></thead>
              <tbody>
                {#each [["Lee Ann", "412", "47"], ["Rüdiger", "96", "114"]] as [n, h, st], i (n)}
                  <tr><td><CellButton current={i === 0} onclick={() => {}}>{n}</CellButton></td><td class="r num">{h}</td><td class="r num">{st}<span class="unit">BB</span></td></tr>
                {/each}
              </tbody>
            </table>
          </Panel>
        </div>
        <Panel title="Panel mit Kopf" sub="Titel, Erklärzeile und Aktionen rechts.">
          {#snippet actions()}<Button size="sm" variant="quiet">Alle ansehen</Button>{/snippet}
          <p class="muted">Inhalt mit 16 px Rand. Mit <code>flush</code> läuft er bis an die Kante, etwa für Tabellen.</p>
          {#snippet footer()}Stand: heute, 21:04{/snippet}
        </Panel>
      </section>
    {/if}

    {#if show("einstellungen")}
      <section id="einstellungen">
        <SectionHead
          title="Einstellungen"
          sub="Eine Zeile je Einstellung: Name und ein Satz links, Bedienung rechts, lange Erklärungen unter „Mehr dazu“. Zeilen stehen in einem Panel mit flush; die Sprung-Leiste führt zu den Gruppen einer langen Seite."
        />
        <Tabs jump label="Gruppen" bind:value={jumpTo} tabs={[{ id: "haende", label: "Hände & Tische" }, { id: "rechnen", label: "Analyse & Rechnen" }, { id: "cloud", label: "Ringside Cloud" }]} />
        <div class="gap"></div>
        <Panel flush>
          <SettingRow title="Neue Hände automatisch analysieren" sub="Sobald du PokerStars schließt, bewertet Ringside deine neuen Hände.">
            {#snippet control()}<Switch label="Neue Hände automatisch analysieren" bind:checked={autoOn} />{/snippet}
          </SettingRow>
          <SettingRow title="Eigene Lösungen teilen" sub="Was dein PC gerechnet hat, hilft anderen – ohne deine Hände.">
            {#snippet control()}<Switch label="Eigene Lösungen teilen" bind:checked={shareOn} />{/snippet}
            {#snippet more()}<p>Die lange Erklärung bleibt vollständig erreichbar, steht aber nicht im Weg.</p>{/snippet}
          </SettingRow>
          <SettingRow title="Bibliothek automatisch erweitern" sub="Gerechnet wird erst, wenn fünf Minuten lang niemand den PC benutzt.">
            {#snippet control()}<SegmentedControl label="Automatik" bind:value={libMode} options={[{ value: "off", label: "Aus" }, { value: "idle", label: "Wenn der PC frei ist" }, { value: "always", label: "Immer" }]} />{/snippet}
          </SettingRow>
          <SettingRow title="Bibliothek mitrechnen" sub="Gesperrt: Während PokerStars läuft, startet Ringside keine Rechnung." locked>
            {#snippet control()}<Button size="sm" disabled>Mitrechnen</Button>{/snippet}
          </SettingRow>
          <SettingRow title="Erkannte Tische" sub="Unter der Zeile steht, was zur Einstellung gehört: Listen, Ergebnisse, Zustände.">
            {#snippet control()}<Button size="sm" variant="quiet">Aktualisieren</Button>{/snippet}
            <EmptyState inline title="Kein PokerStars-Fenster offen" text="Sobald du an einem Tisch sitzt, erscheint er hier." />
          </SettingRow>
          <SettingRow title="Updates" sub="Fehler stehen in der Zeile, in der sie passiert sind.">
            {#snippet control()}<Button size="sm">Nach Updates suchen</Button>{/snippet}
            <EmptyState inline tone="error" title="Die Suche nach Updates hat nicht geklappt" text="Prüf deine Internetverbindung und versuch es gleich noch einmal." />
          </SettingRow>
        </Panel>
        <div class="gap"></div>
        <div class="row"><Switch label="An" checked /> <Switch label="Aus" /> <Switch label="Gesperrt" checked disabled /> <Switch label="Gesperrt aus" disabled /></div>
      </section>
    {/if}

    {#if show("schilder")}
      <section id="schilder">
        <SectionHead title="Schilder" sub="Aktionen in festen Farben, Status als Badge, Herkunft als gestrichelter Hinweis." />
        <h3 class="sub-head">Poker-Aktionen</h3>
        <div class="row">{#each KINDS as k, i (k)}<ActionLabel kind={k} amount={k === "fold" || k === "check" ? undefined : [0, 0, 6, 4.5, 12, 37.5][i]} step={k === "bet" ? 1 : k === "raise" ? 3 : undefined} />{/each}</div>
        <div class="row">{#each KINDS as k (k)}<ActionLabel kind={k} variant="text" />{/each}</div>
        <div class="row bets">{#each [1, 2, 3, 4] as s (s)}<ActionLabel kind="bet" step={s as 1 | 2 | 3 | 4} amount={[1.5, 3, 6, 12][s - 1]} />{/each}<span class="muted">Einsätze werden mit der Größe dunkler.</span></div>
        <h3 class="sub-head">Badges</h3>
        <div class="row">
          <Badge>Neutral</Badge><Badge tone="info">Turnier</Badge><Badge tone="good" dot>Live</Badge><Badge tone="warn">3 Schwachstellen</Badge><Badge tone="bad">Fehler</Badge><Badge size="sm">klein</Badge>
        </div>
        <h3 class="sub-head">Herkunft der Zahlen</h3>
        <div class="row">{#each SOURCES as s (s)}<SourceNote kind={s} />{/each}</div>
        <div class="two">
          <SourceNote kind="equilibrium" variant="block" />
          <SourceNote kind="locked" variant="block" />
        </div>
      </section>
    {/if}

    {#if show("bewertung")}
      <section id="bewertung">
        <SectionHead title="Bewertung" sub="Sechs Stufen: Symbol, Form und Farbe unterscheiden sie – auch ohne Farbe." />
        <div class="mixdemo">
          <p class="muted">Mischung einer Hand: Breite = wie oft, Farbe = Aktion (keine Bewertung).</p>
          <MixBar parts={[{ label: "Check", share: 46, color: "var(--act-check)" }, { label: "Bet 33 % Pot", share: 33, color: "var(--act-bet-1)" }, { label: "Bet 75 % Pot", share: 21, color: "var(--act-bet-3)" }]} />
        </div>
        <div class="grades">
          {#each GRADES as g (g)}
            <div class="grade-col">
              <GradeBadge grade={g} size="lg" />
              <GradeBadge grade={g} cost={GRADE[g].bad ? 1.15 : g === "inaccurate" ? 0.18 : g === "good" ? 0.04 : undefined} />
              <GradeBadge grade={g} size="sm" />
              <GradeBadge grade={g} variant="icon" size="lg" />
            </div>
          {/each}
        </div>
        <div class="verdicts">
          <Verdict grade="best" played="Bet 2,4 BB" context="Flop · Q♠9♦3♥">
            <p>Mit Top Pair willst du Wert von schwächeren Damen und Draws.</p>
          </Verdict>
          <Verdict grade="mix" played="Check" better="Bet 2,4 BB" context="Flop · Q♠9♦3♥">
            <p>Die Lösung checkt hier 45 % und bettet 55 % – beides ist richtig.</p>
          </Verdict>
          <Verdict grade="good" played="Bet 4,8 BB" better="Bet 2,4 BB" cost={0.04} context="Turn · 6♣">
            <p>Die kleine Größe bringt minimal mehr, weil schwächere Hände öfter mitgehen.</p>
          </Verdict>
          <Verdict
            grade="inaccurate"
            played="Call"
            better="Raise auf 14 BB"
            cost={0.18}
            potShare={0.02}
            context="Turn · 6♣"
            source="solver"
            reason="Deine Hand gehört zum schwächsten Drittel deiner Range und hat gegen die Range des Gegners 31 % Equity. Auf Raise auf 14 BB gibt der Gegner in 47 % der Fälle auf. Raise auf 14 BB bringt 0,18 BB mehr als Call."
          >
            <p>Mit deinem Flush-Draw kannst du selbst Druck machen.</p>
          </Verdict>
          <Verdict grade="mistake" played="Bet" better="Check" cost={1.15} potShare={0.12} context="River · Q♠8♠" source="solver">
            {#snippet actions()}<Button size="sm" variant="quiet" kbd="R">Wiederholen</Button><Button size="sm" variant="primary" kbd="Leertaste">Nächste Hand</Button>{/snippet}
            <p>Bessere Hände callen, schlechtere folden – dein Bet gewinnt fast nur gegen nichts. Checken zeigt dir den Showdown billiger.</p>
          </Verdict>
          <Verdict grade="blunder" played="Fold" better="Call" cost={6.4} potShare={0.31} context="River · Q♠8♠" source="model">
            <p>Du brauchst nur 25 % Gewinnchance und hast 48 %. Folden gibt den halben Pot her.</p>
          </Verdict>
        </div>
        <div class="two">
          <Panel title="Session" sub="Wie bei Zug-Listen: Stufe auf einen Blick, Balken je Stufe.">
            <GradeBars counts={{ best: 9, mix: 3, good: 4, inaccurate: 2, mistake: 1, blunder: 0 }} />
            <p class="muted">Mit <code>grades</code> nur die Stufen einer Quelle, etwa die vier des Coaches:</p>
            <GradeBars grades={["best", "good", "inaccurate", "mistake"]} counts={{ best: 2, good: 1, inaccurate: 1, mistake: 1 }} />
            <p class="muted">Mit <code>stacked</code> ein Balken für eine Zusammenfassung:</p>
            <GradeBars stacked counts={{ best: 148, mix: 21, good: 34, inaccurate: 14, mistake: 9, blunder: 4 }} />
            <p class="muted">Eine Zahl über wenige Fälle, jeder Balken wählt seinen Fall (<code>ColumnBars</code>):</p>
            <ColumnBars
              label="Erster am Zug nach Tischgröße"
              selected={kitSize}
              bars={[9, 6, 4, 3, 2].map((n, i) => ({ key: n, value: [15, 24, 37, 50, 98][i], valueText: String([15, 24, 37, 50, 98][i]), label: n === 2 ? "HU" : String(n), title: `${n} Spieler` }))}
              onpick={(k) => (kitSize = Number(k))}
            />
          </Panel>
          <Panel title="Zug-Liste" sub="Symbol trägt die Stufe, nicht nur die Wortfarbe.">
            <ol class="moves">
              {#each [["Preflop", "Raise 2,5 BB", "best"], ["Flop", "Bet 2,4 BB", "mix"], ["Turn", "Check", "inaccurate"], ["River", "Bet 7 BB", "mistake"]] as [street, move, g] (street)}
                <li><span class="muted">{street}</span><b>{move}</b><GradeBadge grade={g as never} size="sm" /></li>
              {/each}
            </ol>
          </Panel>
        </div>
      </section>
    {/if}

    {#if show("karten")}
      <section id="karten">
        <SectionHead title="Karten" sub="Karten sind immer Karten – auch im Text, in Tabellen und Schildern." />
        <div class="row cards-row">
          {#each [22, 30, 40, 52] as s (s)}<PlayingCard card="Qs" size={s} />{/each}
          <PlayingCard card="Th" size={40} /><PlayingCard card="9d" size={40} /><PlayingCard card="8c" size={40} /><PlayingCard hidden size={40} />
        </div>
        <div class="row cards-row">
          <CardSlot label="Flop" tone="next" onpick={() => {}} ariaLabel="Flop-Karte wählen" />
          <CardSlot card="Kd" onpick={() => {}} ariaLabel="Turn ändern" />
          <CardSlot label="River" tone="optional" onpick={() => {}} ariaLabel="River-Karte wählen" />
          <CardSlot tone="idle" />
          <CardSlot size={30} label="?" onpick={() => {}} ariaLabel="Deine Hand wählen" /><CardSlot size={30} card="Ah" onpick={() => {}} ariaLabel="Deine Hand ändern" />
          <span class="muted">Kartenplatz: zum Wählen gestrichelt, „next“ wartet auf die nächste Karte, „optional“ darf leer bleiben.</span>
        </div>
        <p class="card-text"><CardText text="Zwei Paare (Th Tc 9h 9c Js) gegen Straße (Jh Tc 9c 8c 7c)." /></p>
        <p class="card-text small">Handklassen mit <code>hands</code>: <CardText hands text="Mit 8 bb schiebst du K6o immer, 99 und T9s auch; K8+ callt." /></p>
        <p class="card-text small">Notation mit Symbolen: <CardText text="A♠K♠ auf Q♥J♦T♣" /> · einzeln: <MiniCard card="7d" /> <MiniCard card="7d" dim /> (abgeblendet)</p>
        <div class="two">
          <Card><BestHand hole={["Jh", "7c"]} board={["Tc", "9c", "8c", "Ts", "9h"]} winner /></Card>
          <Card><BestHand hole={["Th", "Js"]} board={["Tc", "9c", "8c", "Ts", "9h"]} /></Card>
        </div>
      </section>
    {/if}

    {#if show("tisch")}
      <section id="tisch">
        <SectionHead title="Tisch" sub="Du sitzt unten in der Mitte. Schild und Chips liegen auf der Linie Platz → Mitte, für alle gleich weit." />
        <div class="tables">
          {#key replay}
            <Card pad="sm"><PokerTable seats={SIX} board={["Qs", "9d", "3h"]} pot={13} label="6er-Tisch" /></Card>
            <Card pad="sm"><PokerTable seats={NINE} pot={44} label="9er-Tisch" /></Card>
            <div class="replay-table">
              <Card pad="sm"><PokerTable seats={REPLAY} board={["9c", "4h", "2s", "Kd", "3c"]} pot={0} note="River" label="Nachspielen, kompakt" /></Card>
            </div>
          {/key}
        </div>
        <p class="muted">
          Unter {COMPACT_BELOW} px wird der Tisch kompakt: kleinere Schilder bei gleicher Schrift, Aktion am Schildrand. Beim Nachspielen trägt
          deine Aktion ihre Bewertung, der Gewinner sein Plus.
        </p>
        <Card pad="sm">
          <ActionChain
            streets={[
              { street: "preflop", moves: [{ pos: "LJ", kind: "fold" }, { pos: "HJ", kind: "raise", amount: 2.5 }, { pos: "BTN", kind: "call", amount: 2.5, hero: true }, { pos: "BB", kind: "call", amount: 2.5 }] },
              { street: "flop", cards: ["Qs", "7c", "2d"], moves: [{ pos: "BB", kind: "check" }, { pos: "HJ", kind: "bet", amount: 3.1 }] },
            ]}
            turn={{ pos: "Du · BTN", text: "bist dran" }}
          />
        </Card>
        <div class="two">
          <Card><PotOdds pot={11.2} call={3.1} /></Card>
          <Card><PotOdds pot={88.2} call={58.9} /></Card>
        </div>
        <p class="muted">Kette: was bisher passiert ist, Straße für Straße; wer dran ist, steht am Ende. Pot Odds: dein Call als Teil des Pots danach – so oft musst du gewinnen.</p>
      </section>
    {/if}

    {#if show("raster")}
      <section id="raster">
        <SectionHead title="Range-Raster" sub="Breite = Anteil der Aktion, Höhe = wie viel der Hand in der Range ist. Die Beschriftung steht auf eigenem Schild." />
        <div class="two">
          <div>
            <RangeGrid cells={strategy} label="Beispiel-Strategie" selected={gridSel} onSelect={(i) => (gridSel = i)} onClose={() => (gridSel = -1)}>
              {#snippet tip(i)}<b>{strategy[i] ? "Strategie" : "nicht in der Range"}</b><p class="muted">{strategy[i]?.note ?? "Diese Hand spielt die Range nicht."}</p>{/snippet}
              {#snippet card(i)}<SectionHead level={3} title="Angeheftet" sub={strategy[i]?.note ?? ""} /><Button size="sm" variant="quiet" kbd="Esc" onclick={() => (gridSel = -1)}>Schließen</Button>{/snippet}
            </RangeGrid>
            <div class="legend">{#each [["--act-bet-3", "Raise"], ["--act-call", "Call"], ["--act-fold", "Fold"]] as [v, n] (v)}<span><i style:background="var({v})"></i>{n}</span>{/each}</div>
          </div>
          <div class="stack">
            <h3 class="sub-head">Zustände</h3>
            <RangeGrid cells={stateCells} only={[0, 1, 2, 3, 4, 5, 6]} columns={7} label="Zustände" dimmed={(i) => i === 4} />
            <div class="state-labels">{#each STATE_LABEL as l, k (k)}<span>{l}</span>{/each}</div>
            <h3 class="sub-head">Kontrast: jede Aktionsfarbe × Füllhöhe 0 – 100 %</h3>
            <div class="contrast">
              <div class="fills">{#each FILLS as f (f)}<span class="num">{f * 100} %</span>{/each}</div>
              <div class="spec-rows">
                {#each SPEC_COLORS as v (v)}<code>{v}</code>{/each}
              </div>
              <RangeGrid cells={specimens} only={specimenIdx} columns={FILLS.length} label="Kontrastproben" />
            </div>
            <p class="muted">Gemessen von <code>scripts/check-contrast.mjs</code>: Schrift zu Schild über jeder Füllung mindestens 4,5 : 1.</p>
            <h3 class="sub-head">Klein (Seitenleiste)</h3>
            <div class="small-grid"><RangeGrid cells={strategy} size="sm" label="Kleines Raster" minFill={0.08} /></div>
          </div>
        </div>
      </section>
    {/if}

    {#if show("popups")}
      <section id="popups">
        <SectionHead title="Hinweise & Popups" sub="Term erklärt ein Wort im Satz (auch per Tastatur), Tooltip ein Element, Popover zeigt Details an der Maus, Dialog fragt nach, Toast meldet Ergebnisse." />
        <div class="row">
          <span>Dein <Term tip="Wie oft du freiwillig Geld in den Pot legst. Die Blinds zählen nicht.">VPIP</Term> liegt bei 24 %.</span>
          <Tooltip text="Erscheint auch bei Tastaturfokus." open={only === "popups"}><Button size="sm">Mit Tooltip</Button></Tooltip>
          <Tooltip text="Links daneben, etwa für Begriffe in einer Spalte am rechten Rand." side="left"><Button size="sm">Tooltip links</Button></Tooltip>
          <span bind:this={popHost}><Button size="sm" onclick={(e) => (popAnchor = popAnchor ? null : rectOf(e.currentTarget as Element))}>Popover anheften</Button></span>
          <Button size="sm" onclick={() => (dialogOpen = true)}>Dialog öffnen</Button>
        </div>
        <div class="toasts">
          <Toast title="Gespeichert" />
          <Toast tone="good" title="Analyse fertig" text="12 Hände, 2 Fehler gefunden" action={{ label: "Ansehen", onclick: () => {} }} onclose={() => {}} />
          <Toast tone="bad" title="Import fehlgeschlagen" text="Ordner nicht gefunden. Prüf den Pfad in den Einstellungen." action={{ label: "Einstellungen", onclick: () => {} }} />
        </div>
      </section>
    {/if}

    {#if show("zustaende")}
      <section id="zustaende">
        <SectionHead title="Fortschritt & Zustände" sub="Leer lädt zum Handeln ein, Fehler sagen, was zu tun ist." />
        <div class="row progress">
          <ProgressRing value={0.72} label="Analyse" size={48}>72</ProgressRing>
          <ProgressRing value={0.3} tone="warn" label="Schwachstellen" size={40} />
          <ProgressRing value={null} label="Lädt" size={32} />
          <div class="grow stack">
            <ProgressBar value={0.4} label="Import" />
            <ProgressBar value={0.85} tone="good" label="Ziel" size="md" />
            <ProgressBar value={null} label="Lädt" />
          </div>
        </div>
        <div class="three">
          <Card pad="sm"><EmptyState title="Noch keine Hände" text="Spiel ein paar Hände bei PokerStars – sie erscheinen hier automatisch.">{#snippet action()}<Button variant="primary" size="sm">Ordner prüfen</Button>{/snippet}</EmptyState></Card>
          <Card pad="sm"><EmptyState title="Kein Tisch offen" text="Mit eigenem Symbol für das, was fehlt.">{#snippet icon()}<path d="M12 6.5c4.7 0 8.5 2.5 8.5 5.5s-3.8 5.5-8.5 5.5S3.5 15 3.5 12 7.3 6.5 12 6.5z" />{/snippet}</EmptyState></Card>
          <Card pad="sm"><EmptyState tone="loading" title="Hände werden gelesen" text="Das dauert beim ersten Start etwa eine Minute." /></Card>
          <Card pad="sm"><EmptyState tone="error" title="Ordner nicht gefunden" text="Prüf den Pfad zu deinen Hand-Historien in den Einstellungen.">{#snippet action()}<Button size="sm">Einstellungen öffnen</Button>{/snippet}</EmptyState></Card>
        </div>
      </section>
    {/if}
  </main>
</div>

{#if popAnchor}
  <Popover anchor={popAnchor} pinned label="Beispiel" onclose={() => (popAnchor = null)} side="bottom">
    <SectionHead level={3} title="K♠Q♠ am Button" sub="Raise 2,5 BB · 100 %" />
    <p class="muted"><CardText text="Gegen 3-Bet: Call 62 %, 4-Bet 38 %. Blocker: Ks Qs." /></p>
    <Button size="sm" variant="quiet" kbd="Esc" onclick={() => (popAnchor = null)}>Schließen</Button>
  </Popover>
{/if}

<Dialog bind:open={dialogOpen} title="Session beenden?" sub="Deine Hände bleiben gespeichert.">
  <p>Du hast 14 Entscheidungen gespielt. Die Auswertung findest du danach unter „Lernen“.</p>
  {#snippet actions()}<Button variant="quiet" onclick={() => (dialogOpen = false)}>Weiterspielen</Button><Button variant="primary" onclick={() => (dialogOpen = false)}>Session beenden</Button>{/snippet}
</Dialog>

<svelte:window onkeydown={(e) => e.key === "a" && !(e.target instanceof HTMLInputElement) && ((played = !played), replay++, (ringValue = ringValue > 0.5 ? 0.2 : 0.85))} />

<style>
  .kit {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    min-height: 100vh;
    background: var(--bg);
  }
  .gray {
    filter: grayscale(1);
  }
  .nav {
    position: sticky;
    top: 0;
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    height: 100vh;
    padding: var(--sp-5) var(--sp-4);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-lg);
  }
  .brand span {
    color: var(--text-2);
  }
  .brand b {
    color: var(--text);
  }
  nav {
    display: flex;
    flex-direction: column;
  }
  nav a {
    padding: 6px var(--sp-2);
    border-radius: var(--r-sm);
    color: var(--text-2);
    font-size: var(--fs-md);
    text-decoration: none;
    transition:
      background var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }
  nav a:hover,
  nav a.on {
    background: var(--surface-2);
    color: var(--text);
  }
  .hint {
    margin: auto 0 0;
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  main {
    max-width: 1360px;
    padding: var(--sp-6) var(--sp-8) var(--sp-12);
  }
  .kit-head {
    margin: calc(-1 * var(--sp-6)) calc(-1 * var(--sp-8)) 0;
  }
  .frame {
    overflow: hidden;
    border: 1px solid var(--line-2);
    border-radius: var(--r-md);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-8) 0;
    border-bottom: 1px solid var(--line);
    scroll-margin-top: var(--sp-4);
  }
  h3,
  h4 {
    margin: 0;
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
  }
  h4 {
    margin-bottom: var(--sp-3);
    color: var(--text-2);
  }
  .sub-head {
    margin-top: var(--sp-2);
    color: var(--text-2);
  }
  code {
    color: var(--text-2);
    font-family: ui-monospace, "Cascadia Mono", Menlo, monospace;
    font-size: var(--fs-xs);
  }
  .mixdemo {
    display: grid;
    gap: var(--sp-2);
    max-width: 640px;
    margin-bottom: var(--sp-4);
  }
  .muted {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .grow {
    flex: 1;
    min-width: 160px;
  }
  .gap {
    height: var(--sp-3);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .meters {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-3) var(--sp-4);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .kit-table {
    width: 100%;
    border-collapse: collapse;
  }
  .kit-table th {
    height: 34px;
    padding: 0 var(--sp-4);
    border-bottom: 1px solid var(--line-2);
    text-align: left;
  }
  .kit-table td {
    border-bottom: 1px solid var(--line);
  }
  .kit-table .r {
    padding: 0 var(--sp-4);
    text-align: right;
  }
  .navbar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 10px var(--sp-3);
    border-radius: var(--r);
    background: var(--surface-sunken);
  }
  .navtiles {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-2);
    padding: var(--sp-2);
    border-radius: var(--r);
    background: var(--surface);
  }
  .two {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }
  .three {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-4);
  }
  @media (max-width: 1180px) {
    .two {
      grid-template-columns: minmax(0, 1fr);
    }
    .three {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: var(--sp-4);
  }
  .swatch-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .swatch {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .swatch i {
    width: 28px;
    height: 20px;
    border-radius: var(--r-xs);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }
  .type {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .type li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 110px 150px;
    align-items: baseline;
    gap: var(--sp-3);
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .type li span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .type em {
    color: var(--text-3);
    font-size: var(--fs-sm);
    font-style: normal;
  }
  .space {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .space li {
    display: grid;
    grid-template-columns: 52px 60px 1fr;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-sm);
  }
  .space i {
    height: 10px;
    border-radius: 2px;
    background: var(--text-3);
  }
  .radii,
  .shadows {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }
  .shadows {
    margin-top: var(--sp-4);
  }
  .radii span,
  .shadows span {
    display: grid;
    place-items: center;
    width: 84px;
    height: 52px;
    border: 1px solid var(--line-2);
    background: var(--surface-2);
  }
  .shadows span {
    border-radius: var(--r-md);
  }
  .formats {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px var(--sp-4);
    margin: 0;
  }
  .formats dd {
    margin: 0;
  }

  .curves {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3) var(--sp-6);
  }
  .curve {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2px;
  }
  .curve b {
    font-size: var(--fs-md);
  }
  .curve span {
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .track {
    position: relative;
    height: 22px;
    margin-bottom: var(--sp-1);
    border-radius: var(--r-pill);
    background: var(--surface-2);
  }
  .track i {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text);
    transition-property: left;
    transition-duration: var(--dur-slow);
  }
  .track i.go {
    left: calc(100% - 19px);
  }
  .durs {
    display: flex;
    gap: var(--sp-4);
    align-items: center;
    color: var(--text-2);
    font-size: var(--fs-sm);
  }
  .motion-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-4);
  }
  .motion-grid .muted {
    margin-bottom: var(--sp-3);
  }

  .matrix-table {
    display: grid;
    grid-template-columns: 80px repeat(var(--cols), minmax(0, 1fr));
    align-items: center;
    gap: var(--sp-3);
  }
  .col-head,
  .row-head {
    color: var(--text-3);
    font-size: var(--fs-sm);
  }
  .kbd-sizes {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .bets {
    margin-top: var(--sp-1);
  }

  .grades {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .grade-col {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  .verdicts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-4);
    align-items: start;
  }
  @media (max-width: 1180px) {
    .grades {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .verdicts,
    .motion-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .moves {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .moves li {
    display: grid;
    grid-template-columns: 64px 1fr auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--line);
  }

  .tiles {
    display: grid;
    grid-template-columns: auto repeat(2, 112px);
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    margin: var(--sp-3) 0;
  }
  .tile {
    display: flex;
    justify-content: center;
    gap: 3px;
    font-size: var(--fs-base);
  }
  .cards-row {
    align-items: flex-end;
  }
  .card-text {
    margin: 0;
    font-size: var(--fs-lg);
  }
  .card-text.small {
    font-size: var(--fs-md);
    color: var(--text-2);
  }

  .tables {
    display: grid;
    gap: var(--sp-4);
    max-width: 900px;
  }
  .replay-table {
    width: 620px;
    max-width: 100%;
  }
  .tables :global(.ui-card) {
    background: var(--surface-sunken);
  }

  .legend {
    display: flex;
    gap: var(--sp-4);
    margin-top: var(--sp-3);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .state-labels {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 2px;
    color: var(--text-3);
    font-size: var(--fs-xs);
    text-align: center;
  }
  .contrast {
    display: grid;
    grid-template-columns: 110px minmax(0, 300px);
    grid-template-rows: auto 1fr;
    gap: 4px var(--sp-2);
  }
  .fills {
    grid-column: 2;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    color: var(--text-3);
    font-size: var(--fs-xs);
    text-align: center;
  }
  .spec-rows {
    display: grid;
    grid-auto-rows: 1fr;
    align-items: center;
  }
  .small-grid {
    width: 260px;
  }
  .toasts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }
  .progress {
    gap: var(--sp-6);
  }
  .frame-page {
    margin: var(--sp-4) 330px;
  }
</style>
