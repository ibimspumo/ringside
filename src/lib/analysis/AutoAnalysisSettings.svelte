<script lang="ts">
  import { onMount } from "svelte";
  import { api, on } from "../api";

  let auto = $state<boolean | null>(null);

  onMount(() => {
    api.getSettings().then((s) => (auto = s.analysis.auto));
    const sub = on<{ analysis?: { auto: boolean } }>("settings-changed", (s) => {
      if (s.analysis) auto = s.analysis.auto;
    });
    return () => {
      sub.then((f) => f());
    };
  });

  function change(next: boolean) {
    auto = next;
    api.analysisSetAuto(next);
  }
</script>

<section class="panel">
  <div class="panel-head"><h2 class="panel-title">Hand-Analyse</h2></div>
  <div class="body">
    {#if auto !== null}
      <label class="check">
        <input type="checkbox" checked={auto} onchange={(e) => change(e.currentTarget.checked)} />
        <span>
          Neue Hände automatisch analysieren
          <small>
            Sobald du PokerStars schließt, bewertet Ringside deine neuen Hände. Solange PokerStars läuft, rechnet
            Ringside nie.
          </small>
        </span>
      </label>
    {/if}
  </div>
</section>

<style>
  .body {
    padding: var(--sp-3) 18px 18px;
  }
  small {
    display: block;
    margin-top: 2px;
    color: var(--text-3);
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
</style>
