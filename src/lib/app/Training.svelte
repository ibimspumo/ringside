<script lang="ts">
  import Learn from "../learn/Learn.svelte";
  import type { Filter, HudConfig } from "../types";
  import ClientLock from "./ClientLock.svelte";
  import type { LearnTarget } from "./Leaks.svelte";
  import type { StepHands } from "../next/types";

  let {
    hud = null,
    filter,
    open = null,
    onOpened = () => {},
    onStepHands,
  }: { hud?: HudConfig | null; filter: Filter; open?: LearnTarget | null; onOpened?: () => void; onStepHands?: (hands: StepHands) => void } = $props();
</script>

<ClientLock>
  <Learn {hud} {filter} {open} {onOpened} onHands={onStepHands} />
</ClientLock>
