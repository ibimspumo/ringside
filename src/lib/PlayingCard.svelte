<script lang="ts">
  // The card lives in the kit; this path stays until the views import it from there.
  import PlayingCard from "./ui/PlayingCard.svelte";

  let props: { card?: string; size?: number; hidden?: boolean } = $props();
</script>

<PlayingCard {...props} />
