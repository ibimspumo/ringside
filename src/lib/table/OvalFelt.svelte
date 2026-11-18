<script lang="ts">
  import type { TableLayout } from "./seats";

  /** The felt and rail of a table laid out by `seats.ts`: the seats sit on its edge. */
  let { felt }: { felt: TableLayout["felt"] } = $props();
</script>

<div class="rail" style="left:{felt.cx - felt.rx - 10}px; top:{felt.cy - felt.ry - 10}px; width:{2 * felt.rx + 20}px; height:{2 * felt.ry + 20}px">
  <div class="cloth"></div>
</div>

<style>
  .rail {
    position: absolute;
    border-radius: 50%;
    padding: 10px;
    background: var(--plate);
    box-shadow:
      inset 0 0 0 1px var(--plate-edge),
      var(--shadow-3);
  }
  .cloth {
    height: 100%;
    border-radius: 50%;
    background: radial-gradient(ellipse at 50% 40%, var(--felt) 0%, var(--felt-edge) 100%);
    box-shadow: inset 0 0 0 1px var(--felt-edge);
  }
</style>
