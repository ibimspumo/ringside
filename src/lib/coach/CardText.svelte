<script lang="ts" module>
  type Part = { text: string } | { ranks: string[]; kind: string } | { cards: string[] };

  const RANK = "AKQJT98765432";
  /** Hand classes (K6o, 99, T9s, AK) and exact cards (Kh, AsKd); not inside numbers or before units. */
  const TOKEN = /(?<![\w,.])((?:[AKQJT2-9][shdc]){1,2}|[AKQJT2-9]{2}[so]?\+?)(?![\w+]|[,.]\d|\s?(?:bb|BB|%|\$|Tsd))/g;

  function split(text: string): Part[] {
    const parts: Part[] = [];
    let at = 0;
    for (const m of text.matchAll(TOKEN)) {
      const t = m[1];
      const exact = /^(?:[AKQJT2-9][shdc])+$/.test(t);
      // Two digits alone ("22") are more often a number than pocket deuces.
      if (!exact && /^\d\d$/.test(t) && !/^(\d)\1$/.test(t)) continue;
      if (!exact && ![...t.slice(0, 2)].every((c) => RANK.includes(c))) continue;
      if (m.index! > at) parts.push({ text: text.slice(at, m.index) });
      parts.push(exact ? { cards: t.match(/../g)! } : { ranks: [t[0], t[1]], kind: t.slice(2) });
      at = m.index! + t.length;
    }
    if (at < text.length) parts.push({ text: text.slice(at) });
    return parts;
  }
</script>

<script lang="ts">
  import PlayingCard from "../PlayingCard.svelte";

  /** Text with poker hands drawn as small cards. */
  let { text }: { text: string } = $props();

  const parts = $derived(split(text));
  const KIND: Record<string, string> = { s: "suited, also gleiche Farbe", o: "offsuit, also verschiedene Farben", "+": "und besser" };
  const label = (p: { ranks: string[]; kind: string }) =>
    `${p.ranks.join("")}${[...p.kind].map((k) => (KIND[k] ? `, ${KIND[k]}` : "")).join("")}`;
</script>

{#each parts as p, i (i)}{#if "text" in p}{p.text}{:else if "cards" in p}<span class="cards"
      >{#each p.cards as c, k (k)}<PlayingCard card={c} size={17} />{/each}</span
    >{:else}<span class="hand" title={label(p)}
      >{#each p.ranks as r, k (k)}<b>{r === "T" ? "10" : r}</b>{/each}{#if p.kind}<i>{p.kind}</i>{/if}</span
    >{/if}{/each}

<style>
  .cards,
  .hand {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    vertical-align: -0.2em;
    margin: 0 1px;
  }
  .hand {
    cursor: help;
  }
  .hand b {
    display: inline-grid;
    place-items: center;
    min-width: 1.2em;
    height: 1.45em;
    padding: 0 2px;
    border-radius: 3px;
    background: var(--text);
    color: var(--bg);
    font-family: var(--font-num);
    font-size: 0.86em;
    font-weight: 700;
    line-height: 1;
  }
  .hand i {
    align-self: center;
    margin-left: 2px;
    font-style: normal;
    font-size: 0.8em;
    font-weight: 600;
    line-height: 1;
    color: var(--text-2);
  }
</style>
