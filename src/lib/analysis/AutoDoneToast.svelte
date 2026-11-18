<script lang="ts">
  import { onMount } from "svelte";
  import { onAnalysisAutoDone } from "../api";
  import { adviceLocked, watchPokerClient } from "../app/client.svelte";
  import { silence } from "../jobs/store.svelte";
  import Toast from "../ui/Toast.svelte";
  import type { AutoDone } from "./types";

  const SHOW_MS = 12000;

  /** Opens the report of the finished tournament, or else "Meine teuersten Fehler" on the hands page. */
  let { onView }: { onView: (tournament: number | null) => void } = $props();

  watchPokerClient();
  let done = $state<AutoDone | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const hands = (n: number) => (n === 1 ? "1 neue Hand bewertet" : `${n.toLocaleString("de-DE")} neue Hände bewertet`);
  const mistakes = (n: number) => (n === 0 ? "Keine teuren Fehler" : n === 1 ? "1 teurer Fehler" : `${n.toLocaleString("de-DE")} teure Fehler`);

  function close() {
    clearTimeout(timer);
    done = null;
  }

  onMount(() => {
    const sub = onAnalysisAutoDone((d) => {
      if (d.job !== null) silence(d.job);
      clearTimeout(timer);
      done = d;
      timer = setTimeout(close, SHOW_MS);
    });
    return () => {
      clearTimeout(timer);
      sub.then((f) => f());
    };
  });
</script>

{#if done && !adviceLocked()}
  <Toast
    tone="good"
    title={hands(done.hands)}
    text="{mistakes(done.mistakes)} · {done.tournament !== null ? 'dein Turnier-Bericht ist fertig' : 'gerechnet, nachdem du PokerStars geschlossen hast'}"
    action={{
      label: done.tournament !== null ? "Bericht ansehen" : "Ansehen",
      onclick: () => {
        const tournament = done?.tournament ?? null;
        close();
        onView(tournament);
      },
    }}
    onclose={close}
  />
{/if}
