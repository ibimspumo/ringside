<script lang="ts">
  import ClientLock from "./ClientLock.svelte";

  const COMING = [
    ["Push/Fold", "Wann All-in oder Fold richtig ist, nach Nash und ICM."],
    ["Equity", "Hand gegen Range schätzen und sofort nachrechnen."],
    ["Deine Spots", "Entscheidungen aus deinen eigenen Händen noch einmal spielen."],
  ];
</script>

<ClientLock>
  <div class="page">
    <section class="panel">
      <div class="intro">
        <h2>Der Lernbereich kommt bald</h2>
        <p class="dim">
          Hier übst du bald mit Rechnern und Aufgaben, die auf deinen eigenen Händen aufbauen. Solange PokerStars läuft,
          bleibt dieser Bereich gesperrt.
        </p>
      </div>
      <ul>
        {#each COMING as [name, text] (name)}
          <li>
            <strong>{name}</strong>
            <span class="dim">{text}</span>
          </li>
        {/each}
      </ul>
    </section>
  </div>
</ClientLock>

<style>
  .page {
    padding-top: 20px;
  }
  .panel {
    max-width: 720px;
  }
  .intro {
    padding: 18px 20px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin: 0 0 6px;
    font-family: var(--font-num);
    font-size: 18px;
    font-weight: 600;
  }
  .intro p {
    margin: 0;
    max-width: 60ch;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 140px minmax(0, 1fr);
    gap: 16px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--line);
    font-size: 13.5px;
  }
  li:last-child {
    border-bottom: 0;
  }
  strong {
    font-weight: 600;
  }
  @container (max-width: 640px) {
    li {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
