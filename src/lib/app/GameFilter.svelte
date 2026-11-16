<script lang="ts">
  import type { GameKind } from "../types";

  let { value = $bindable() }: { value: GameKind | null } = $props();
  const OPTIONS: [GameKind | null, string][] = [
    ["cash", "Cash Games"],
    ["tournament", "Turniere"],
    [null, "Alle"],
  ];
</script>

<div class="seg">
  {#each OPTIONS as [id, label] (label)}
    <button class:on={value === id} onclick={() => (value = id)}>{label}</button>
  {/each}
</div>
