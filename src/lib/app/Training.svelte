<script lang="ts">
  import Learn from "../learn/Learn.svelte";
  import type { HudConfig } from "../types";
  import ClientLock from "./ClientLock.svelte";

  let { hud = null }: { hud?: HudConfig | null } = $props();
</script>

<ClientLock feature="Der Lernbereich">
  <Learn {hud} />
</ClientLock>
