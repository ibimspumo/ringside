<script lang="ts">
  import type { Snippet } from "svelte";

  let { children }: { children?: Snippet } = $props();
</script>

<div class="felt">
  <div class="rail">
    <div class="cloth"></div>
  </div>
  {@render children?.()}
</div>

<style>
  .felt {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: #0f1114;
  }
  .rail {
    position: absolute;
    /* Proportions of the client's table at its default window shape. */
    left: 13%;
    right: 13%;
    top: 17%;
    bottom: 30%;
    border-radius: 999px;
    padding: 1.3%;
    background: #1c1f24;
    box-shadow:
      inset 0 0 0 1px rgba(236, 235, 231, 0.06),
      0 18px 40px rgba(0, 0, 0, 0.45);
  }
  .cloth {
    height: 100%;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 42%, #23312d 0%, #1b2623 62%, #16201d 100%);
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.4),
      inset 0 0 28px rgba(0, 0, 0, 0.35);
  }
</style>
