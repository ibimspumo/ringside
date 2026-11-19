<script lang="ts">
  import type { Site } from "../types";

  /** Small mark for the poker client a hand, tournament or session comes from ("PS", "GG"). */
  let { site, size = "sm" }: { site: Site; size?: "sm" | "md" } = $props();

  const SITES: Record<Site, { short: string; name: string }> = {
    pokerstars: { short: "PS", name: "PokerStars" },
    ggpoker: { short: "GG", name: "GGPoker" },
  };
  const s = $derived(SITES[site] ?? SITES.pokerstars);
</script>

<span class="ui-site {site} {size}" title="Aus {s.name}" role="img" aria-label={s.name}>{s.short}</span>

<style>
  .ui-site {
    --c: var(--site-ps);
    display: inline-grid;
    place-items: center;
    flex: none;
    min-width: 22px;
    height: 16px;
    padding: 0 4px;
    border-radius: var(--r-xs);
    border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
    background: color-mix(in srgb, var(--c) 14%, transparent);
    color: var(--c);
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    font-weight: var(--fw-semibold);
    letter-spacing: 0.04em;
    line-height: 1;
    vertical-align: middle;
    cursor: default;
  }
  .ggpoker {
    --c: var(--site-gg);
  }
  .md {
    min-width: 28px;
    height: 20px;
    padding: 0 6px;
    font-size: var(--fs-sm);
  }
</style>
