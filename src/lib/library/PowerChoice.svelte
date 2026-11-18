<script lang="ts">
  import SegmentedControl from "../ui/SegmentedControl.svelte";
  import { libraryApi } from "./api";
  import { threadsFor } from "./labels";
  import type { LibraryOverview, LibraryPower } from "./types";

  /** How much of the computer the library may use while it solves. */
  let { data }: { data: LibraryOverview } = $props();

  let error = $state<string | null>(null);

  const POWERS: { value: LibraryPower; label: string; text: string }[] = [
    { value: "gentle", label: "Sanft", text: "Leise und sparsam. Dauert länger, der PC bleibt kühl." },
    { value: "normal", label: "Normal", text: "Empfohlen. Schnell genug, und der PC bleibt flott." },
    { value: "full", label: "Volle Kraft", text: "Am schnellsten, braucht aber deutlich mehr Strom, und der Lüfter wird hörbar." },
  ];
  const power = $derived(POWERS.find((p) => p.value === data.power) ?? POWERS[1]);

  function setPower(p: LibraryPower) {
    data.power = p;
    libraryApi.setPower(p).catch((e) => (error = String(e)));
  }
</script>

<div class="power">
  <SegmentedControl label="Kraft" value={data.power} options={POWERS} onchange={setPower} />
  <p>
    <b class="num">{threadsFor(data.power, data.cores)} von {data.cores}</b> Rechenkernen. {power.text}
    <span class="faint">Gilt ab dem nächsten Spot; Arbeitsspeicher höchstens ein Viertel.</span>
  </p>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</div>

<style>
  .power {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-2);
  }
  p {
    margin: 0;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-base);
    max-width: 68ch;
  }
  b {
    color: var(--text);
    font-weight: var(--fw-semibold);
  }
  .err {
    color: var(--neg);
  }
</style>
