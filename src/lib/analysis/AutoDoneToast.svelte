<script lang="ts">
  import { onMount } from "svelte";
  import { onAnalysisAutoDone } from "../api";
  import { adviceLocked, watchPokerClient } from "../app/client.svelte";
  import { silence } from "../jobs/store.svelte";
  import { delta, points } from "../skill/labels";
  import { loadSkill } from "../skill/store.svelte";
  import Toast from "../ui/Toast.svelte";
  import type { AutoDone } from "./types";

  const SHOW_MS = 12000;

  /** Opens the session review, which also shows the change of the Spielwertung. */
  let { onView }: { onView: () => void } = $props();

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
      loadSkill();
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
  {@const s = done.skill}
  <Toast
    tone="good"
    title={hands(done.hands)}
    text={s ? `Wertung ${s.delta !== null ? `${delta(s.delta)}, jetzt ` : ""}${points(s.rating)} · ${mistakes(done.mistakes)}` : `${mistakes(done.mistakes)} · dein Session-Rückblick ist fertig`}
    action={{
      label: "Rückblick ansehen",
      onclick: () => {
        close();
        onView();
      },
    }}
    onclose={close}
  />
{/if}
