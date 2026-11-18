<script lang="ts">
  import { bb } from "./format";
  import { ACTION_NAME, actionColor, type ActionKind, type SizeStep } from "./poker";

  /**
   * A poker action, the same for every player and every view: colour mark, word, amount in BB.
   * `tag` is the plate at a seat, `text` the inline form for lists and sentences.
   */
  let {
    kind,
    amount,
    step,
    variant = "tag",
    size = "md",
  }: {
    kind: ActionKind;
    /** In big blinds; omitted for fold and check. */
    amount?: number;
    step?: SizeStep;
    variant?: "tag" | "text";
    size?: "sm" | "md";
  } = $props();

  const color = $derived(actionColor(kind, step));
</script>

<span class="ui-action {variant} {size}" style:--c={color}>
  <i aria-hidden="true"></i>{ACTION_NAME[kind]}{#if amount !== undefined && kind !== "fold" && kind !== "check"}<b class="num">{bb(amount)}</b>{/if}
</span>

<style>
  .ui-action {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    color: var(--text);
    font-size: var(--fs-md);
    font-weight: var(--fw-medium);
    line-height: 1;
  }
  .tag {
    height: 24px;
    padding: 0 var(--sp-2) 0 6px;
    border-radius: var(--r-sm);
    border: 1px solid color-mix(in srgb, var(--c) 55%, transparent);
    background: color-mix(in srgb, var(--c) 20%, var(--surface-2));
    box-shadow: var(--shadow-1);
  }
  .sm {
    font-size: var(--fs-sm);
  }
  .tag.sm {
    height: 20px;
    padding: 0 6px 0 5px;
  }
  i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  b {
    font-weight: var(--fw-semibold);
  }
</style>
