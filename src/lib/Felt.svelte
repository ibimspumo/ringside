<script lang="ts">
  import type { Snippet } from "svelte";

  let { children }: { children?: Snippet } = $props();
</script>

<div class="felt">
  <div class="rail">
    <div class="cloth">
      <div class="mark">RINGSIDE</div>
    </div>
  </div>
  {@render children?.()}
</div>

<style>
  .felt {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background:
      radial-gradient(ellipse at 50% 40%, rgba(255, 138, 61, 0.07), transparent 60%),
      radial-gradient(ellipse at 50% 120%, rgba(76, 198, 255, 0.08), transparent 55%),
      #07090d;
  }
  .rail {
    position: absolute;
    left: 12%;
    right: 12%;
    top: 17%;
    bottom: 22%;
    border-radius: 999px;
    padding: 1.4%;
    background: linear-gradient(180deg, #2a2f3b, #12151c);
    box-shadow:
      0 30px 60px rgba(0, 0, 0, 0.6),
      0 0 0 1px rgba(255, 255, 255, 0.06) inset;
  }
  .cloth {
    height: 100%;
    border-radius: 999px;
    background: radial-gradient(ellipse at 50% 35%, #1d4a5e 0%, #12303f 55%, #0b1d28 100%);
    box-shadow: 0 0 40px rgba(0, 0, 0, 0.6) inset;
    display: grid;
    place-items: center;
  }
  .mark {
    font-family: var(--font-num);
    letter-spacing: 0.5em;
    font-size: clamp(10px, 1.6vw, 20px);
    color: rgba(255, 255, 255, 0.07);
    font-weight: 700;
  }
</style>
