<script lang="ts">
  /** Progress as a thin bar; `value` null runs while the end is unknown. */
  let {
    value,
    tone = "neutral",
    size = "sm",
    label,
  }: {
    value: number | null;
    tone?: "neutral" | "good" | "warn" | "bad" | "info";
    size?: "sm" | "md";
    label: string;
  } = $props();

  const v = $derived(value === null ? 0 : Math.max(0, Math.min(1, value)));
</script>

<div
  class="ui-bar {tone} {size}"
  class:busy={value === null}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={value === null ? undefined : Math.round(v * 100)}
>
  <i style:width={value === null ? "32%" : `${v * 100}%`}></i>
</div>

<style>
  .ui-bar {
    --c: var(--text);
    position: relative;
    overflow: hidden;
    height: 4px;
    border-radius: var(--r-pill);
    background: var(--surface-3);
  }
  .md {
    height: 8px;
  }
  .good {
    --c: var(--pos);
  }
  .warn {
    --c: var(--amber);
  }
  .bad {
    --c: var(--neg);
  }
  .info {
    --c: var(--info);
  }
  i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: inherit;
    background: var(--c);
    transition: width var(--dur) var(--ease-standard);
  }
  .busy i {
    animation: run var(--dur-loop) var(--ease-standard) infinite;
  }
  @keyframes run {
    from {
      left: -32%;
    }
    to {
      left: 100%;
    }
  }
</style>
