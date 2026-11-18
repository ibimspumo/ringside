<script lang="ts">
  import Learn from "../learn/Learn.svelte";
  import type { HudConfig } from "../types";
  import ClientLock from "./ClientLock.svelte";
  import type { LearnTarget } from "./Leaks.svelte";

  let {
    hud = null,
    open = null,
    onOpened = () => {},
    onHands,
  }: { hud?: HudConfig | null; open?: LearnTarget | null; onOpened?: () => void; onHands?: (key: string, label: string) => void } = $props();
</script>

<ClientLock feature="Der Lernbereich">
  <Learn {hud} {open} {onOpened} {onHands} />
</ClientLock>
