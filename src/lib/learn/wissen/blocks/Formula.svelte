<script lang="ts">
  let {
    name,
    num,
    den,
    after,
    example,
    heuristic = false,
  }: {
    name: string;
    num: string;
    den?: string;
    after?: string;
    example?: { given: string; calc: string; result: string };
    heuristic?: boolean;
  } = $props();
</script>

<section class="formula">
  <header>
    <span>Formel</span>
    {#if heuristic}<span class="badge">Faustregel</span>{/if}
  </header>
  <div class="eq">
    <span class="name">{name} =</span>
    {#if den}
      <span class="frac"><span>{num}</span><span class="den">{den}</span></span>
    {:else}
      <span>{num}</span>
    {/if}
    {#if after}<span>{after}</span>{/if}
  </div>
  {#if example}
    <div class="ex">
      <span class="given">{example.given}</span>
      <span class="calc num">{example.calc}</span>
      <b class="res num">{example.result}</b>
    </div>
  {/if}
</section>

<style>
  .formula {
    display: grid;
    gap: 12px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .badge {
    padding: 1px 7px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    color: var(--text-2);
    font-weight: 500;
  }
  .eq {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    font-size: 15px;
  }
  .name {
    color: var(--text-2);
  }
  .frac {
    display: inline-grid;
    text-align: center;
  }
  .frac .den {
    border-top: 1.5px solid var(--text-2);
    padding-top: 2px;
    margin-top: 2px;
  }
  .ex {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 2px 12px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
  }
  .given {
    grid-column: 1;
    color: var(--text-2);
    font-size: 13px;
  }
  .calc {
    grid-column: 1;
    color: var(--text-3);
    font-size: 13.5px;
  }
  .res {
    grid-column: 2;
    grid-row: 1 / 3;
    align-self: center;
    font-size: 22px;
    font-weight: 600;
  }
</style>
